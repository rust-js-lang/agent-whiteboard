//! One session boundary for human controls and native WebMCP tools.
use react::{Dispatch, SetStateAction, dom::flush_sync};
use serde::Serialize;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use webapi::{AbortSignal, BroadcastChannel};
use whiteboard_core::{Command, Journal, Receipt, Snapshot};

use crate::storage::{Storage, Stored};
use crate::surface::Surface;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Submission {
    pub receipt: Receipt,
    pub accepted: bool,
    pub current_sequence: u32,
    pub presented: bool,
}
#[derive(Clone, Copy, PartialEq)]
pub struct SessionInfo {
    pub ready: bool,
    pub mode: &'static str,
    pub connected: bool,
}
impl Default for SessionInfo {
    fn default() -> Self {
        Self {
            ready: false,
            mode: "browser",
            connected: true,
        }
    }
}
pub struct Session {
    storage: RefCell<Option<Storage>>,
    journal: RefCell<Journal>,
    actor: RefCell<String>,
    mode: Cell<&'static str>,
    connected: Cell<bool>,
    generation: Cell<u32>,
    surface: RefCell<Option<Rc<Surface>>>,
    channel: RefCell<Option<&'static BroadcastChannel>>,
    info: &'static Dispatch<SetStateAction<SessionInfo>>,
    publish: &'static Dispatch<SetStateAction<Snapshot>>,
    status: &'static Dispatch<SetStateAction<String>>,
    saving: &'static Dispatch<SetStateAction<u32>>,
}
impl Session {
    pub fn new(
        publish: &'static Dispatch<SetStateAction<Snapshot>>,
        status: &'static Dispatch<SetStateAction<String>>,
        saving: &'static Dispatch<SetStateAction<u32>>,
        info: &'static Dispatch<SetStateAction<SessionInfo>>,
    ) -> Self {
        Self {
            storage: RefCell::new(None),
            journal: RefCell::new(Journal::default()),
            actor: RefCell::new("initial".into()),
            mode: Cell::new("browser"),
            connected: Cell::new(true),
            generation: Cell::new(0),
            surface: RefCell::new(None),
            channel: RefCell::new(None),
            info,
            publish,
            status,
            saving,
        }
    }
    fn publish_info(&self) {
        self.info.set(SessionInfo {
            ready: self.ready(),
            mode: self.mode(),
            connected: self.connected(),
        });
    }
    pub fn ready(&self) -> bool {
        self.storage.borrow().is_some()
    }
    pub fn mode(&self) -> &'static str {
        self.mode.get()
    }
    pub fn connected(&self) -> bool {
        self.connected.get()
    }
    pub fn actor(&self) -> String {
        self.actor.borrow().clone()
    }
    pub fn snapshot(&self) -> Snapshot {
        self.journal.borrow().snapshot(&self.actor.borrow())
    }
    pub fn journal(&self) -> Journal {
        self.journal.borrow().clone()
    }
    pub fn attach(&self, surface: Rc<Surface>) {
        surface.update(self.snapshot());
        *self.surface.borrow_mut() = Some(surface);
    }
    pub fn surface(&self) -> Result<Rc<Surface>, String> {
        self.surface
            .borrow()
            .clone()
            .ok_or_else(|| "The drawing surface is still opening.".into())
    }
    pub fn detach(&self) {
        let surface = self.surface.borrow_mut().take();
        if let Some(surface) = surface {
            surface.stop_replay();
        }
    }
    pub fn set_channel(&self, channel: Option<&'static BroadcastChannel>) {
        *self.channel.borrow_mut() = channel;
    }
    pub async fn initialize(&self) -> Result<(), String> {
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        let storage = Storage::open().await?;
        let stored = storage.read().await?;
        if self.generation.get() != generation {
            storage.close();
            return Ok(());
        }
        self.mode.set(stored.mode);
        *self.storage.borrow_mut() = Some(storage);
        self.present(stored);
        Ok(())
    }
    fn backend(&self) -> Result<Storage, String> {
        self.storage
            .borrow()
            .clone()
            .ok_or_else(|| "The board is still opening. Check its storage status.".into())
    }
    fn present(&self, stored: Stored) {
        if stored.journal.sequence < self.journal.borrow().sequence {
            return;
        }
        *self.actor.borrow_mut() = stored.actor;
        self.mode.set(stored.mode);
        self.connected.set(true);
        self.status.set(stored.status.into());
        self.publish_info();
        *self.journal.borrow_mut() = stored.journal;
        let snapshot = self.snapshot();
        let surface = self.surface.borrow().clone();
        let publish = self.publish;
        flush_sync(move || {
            if let Some(surface) = surface {
                surface.update(snapshot.clone());
            }
            publish.set(snapshot);
        });
    }
    pub async fn refresh(&self) -> Result<(), String> {
        let backend = self.backend()?;
        match backend.read().await {
            Ok(stored) => {
                self.present(stored);
                Ok(())
            }
            Err(error) => {
                self.connected.set(false);
                self.status.set("Disconnected".into());
                self.publish_info();
                Err(error)
            }
        }
    }
    pub async fn submit(
        &self,
        command: Command,
        signal: Option<&'static AbortSignal>,
    ) -> Result<Submission, String> {
        self.saving.update(|value| value + 1);
        let result = self.commit(command, signal).await;
        self.saving.update(|value| value.saturating_sub(1));
        result
    }
    async fn commit(
        &self,
        command: Command,
        signal: Option<&'static AbortSignal>,
    ) -> Result<Submission, String> {
        let stored = self.backend()?.submit(command, signal).await?;
        let receipt = stored
            .receipt
            .clone()
            .ok_or("Storage returned no command receipt.")?;
        let accepted = stored.accepted;
        self.present(stored);
        if accepted
            && self.mode.get() == "browser"
            && let Some(channel) = *self.channel.borrow()
        {
            channel.post_message("committed");
        }
        let surface = self.surface.borrow().clone();
        let presented = if let Some(surface) = surface {
            surface.wait_painted(receipt.sequence).await
        } else {
            false
        };
        Ok(Submission {
            receipt,
            accepted,
            current_sequence: self.journal.borrow().sequence,
            presented,
        })
    }
    pub fn close(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        let storage = self.storage.borrow_mut().take();
        if let Some(storage) = storage {
            storage.close();
        }
    }
}
