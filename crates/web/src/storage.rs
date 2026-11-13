//! The persistence adapter owns atomic acceptance. It never publishes an uncommitted proposal.
use std::cell::RefCell;
use std::panic::catch_unwind;
use std::rc::Rc;

use js::{promise, settle};
use serde::Deserialize;
use webapi::{AbortSignal, BodyInit, RequestInit, headers, window};
use whiteboard_core::{Command, Journal, Receipt};

use crate::idb::{self, Database, TransactionOptions};
const STORE: &str = "boards";
const KEY: &str = "local";
const MAX_BYTES: usize = 32 * 1024 * 1024;
#[derive(Clone)]
pub enum Storage {
    Browser {
        db: &'static Database,
        actor: String,
    },
    Temporary {
        journal: Rc<RefCell<Journal>>,
        actor: String,
    },
    Server {
        room: String,
    },
}
pub struct Stored {
    pub journal: Journal,
    pub actor: String,
    pub mode: &'static str,
    pub status: &'static str,
    pub receipt: Option<Receipt>,
    pub accepted: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Remote {
    record: String,
    actor: String,
    receipt: Option<Receipt>,
    accepted: Option<bool>,
}
fn actor_id() -> String {
    catch_unwind(|| {
        let storage = window.local_storage().ok();
        if let Some(id) = storage.and_then(|storage| storage.get_item("agent-whiteboard.actor"))
            && !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_:.".contains(&c))
        {
            return id;
        }
        let id = window.crypto().random_uuid();
        if let Some(storage) = storage {
            let _ = storage.set_item("agent-whiteboard.actor", &id);
        }
        id
    })
    .unwrap_or_else(|_| window.crypto().random_uuid())
}
pub fn decode(text: Option<String>) -> Result<Journal, String> {
    let journal: Journal = match text {
        None => Journal::default(),
        Some(text) if text.len() <= MAX_BYTES => serde_json::from_str(&text)
            .map_err(|e| format!("Cannot read saved board: {e}. Saved data was preserved."))?,
        Some(_) => {
            return Err("Saved board exceeds the supported size. It has been preserved.".into());
        }
    };
    journal.validate()?;
    Ok(journal)
}
fn cancelled(signal: Option<&AbortSignal>) -> Result<(), String> {
    if signal.is_some_and(|signal| signal.aborted()) {
        Err("Request cancelled before acceptance.".into())
    } else {
        Ok(())
    }
}
pub async fn request(
    url: &str,
    body: Option<String>,
    signal: Option<&'static AbortSignal>,
) -> Result<String, String> {
    cancelled(signal)?;
    let headers = headers::new(None);
    if body.is_some() {
        headers.set("Content-Type", "application/json");
    }
    let response = settle(window.fetch(
        url,
        Some(RequestInit {
            method: Some(if body.is_some() { "POST" } else { "GET" }),
            headers: Some(headers),
            body: body.as_deref().map(|body| Some(BodyInit::Str(body))),
            credentials: Some("same-origin"),
            cache: Some("no-store"),
            signal: signal.map(Some),
            ..RequestInit::default()
        }),
    ))
    .await
    .map_err(|_| {
        "Disconnected. No server acknowledgement was received; retry the same command ID."
            .to_string()
    })?;
    if response.status() == 401.0 {
        return Err("LOGIN_REQUIRED".into());
    }
    let text = settle(response.text())
        .await
        .map_err(|_| "Could not read the server response.".to_string())?;
    if !response.ok() {
        let error: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        return Err(error
            .get("error")
            .and_then(|value| value.as_str())
            .unwrap_or("Server rejected this request.")
            .into());
    }
    Ok(text)
}
pub async fn login(token: &str) -> Result<(), String> {
    request(
        "./api/session",
        Some(serde_json::json!({"token":token}).to_string()),
        None,
    )
    .await
    .map(|_| ())
}
impl Storage {
    pub async fn open() -> Result<Self, String> {
        if let Some(room) = window.location().hash().strip_prefix("#room=") {
            if room.is_empty()
                || room.len() > 64
                || !room
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            {
                return Err("Invalid room ID.".into());
            }
            return Ok(Self::Server { room: room.into() });
        }
        let actor = actor_id();
        let opening = catch_unwind(|| {
            idb::open_db(
                "agent-whiteboard",
                1,
                idb::OpenOptions {
                    upgrade: |db: &'static Database, _, _, _, _| {
                        db.create_object_store(STORE);
                    },
                },
            )
        });
        match opening {
            Ok(opening) => match settle(opening).await {
                Ok(db) => {
                    db.set_on_version_change(move |_| db.close());
                    Ok(Self::Browser { db, actor })
                }
                Err(error) if error.name() == "VersionError" => Err(
                    "A newer app saved this board. Update the app; saved data was preserved."
                        .into(),
                ),
                Err(error)
                    if error.name() == "SecurityError" || error.name() == "NotSupportedError" =>
                {
                    Ok(Self::Temporary {
                        journal: Rc::new(RefCell::new(Journal::default())),
                        actor,
                    })
                }
                Err(_) => Err("Cannot open saved boards. Storage was not replaced.".into()),
            },
            Err(_) => Ok(Self::Temporary {
                journal: Rc::new(RefCell::new(Journal::default())),
                actor,
            }),
        }
    }
    fn stored(&self, journal: Journal, receipt: Option<Receipt>, accepted: bool) -> Stored {
        let (actor, mode, status) = match self {
            Self::Browser { actor, .. } => (actor.clone(), "browser", "Saved in this browser"),
            Self::Temporary { actor, .. } => (
                actor.clone(),
                "temporary",
                "Temporary · export to keep your work",
            ),
            Self::Server { .. } => unreachable!(),
        };
        Stored {
            journal,
            actor,
            mode,
            status,
            receipt,
            accepted,
        }
    }
    async fn transact(
        &self,
        command: Option<Command>,
        signal: Option<&'static AbortSignal>,
    ) -> Result<Stored, String> {
        cancelled(signal)?;
        match self {
            Self::Temporary { journal, actor } => {
                let current = journal.borrow().clone();
                if let Some(command) = command {
                    let accepted = current.accept(command, actor)?;
                    *journal.borrow_mut() = accepted.journal.clone();
                    Ok(self.stored(accepted.journal, Some(accepted.receipt), accepted.accepted))
                } else {
                    Ok(self.stored(current, None, false))
                }
            }
            Self::Browser { db, actor } => {
                let mode = if command.is_some() {
                    "readwrite"
                } else {
                    "readonly"
                };
                let transaction = catch_unwind(|| {
                    db.transaction(
                        STORE,
                        mode,
                        TransactionOptions {
                            durability: "strict",
                        },
                    )
                })
                .map_err(|_| "Saved connection is closed. Reload before continuing.".to_string())?;
                let completion=transaction.done().then_with(|_|Ok(()),|_|Err("Saving failed. The board was not committed; retry the same command ID.".to_string()));
                let operation = promise(async move {
                    let store = transaction.object_store(STORE);
                    let text = settle(store.get::<String>(KEY))
                        .await
                        .map_err(|_| "Cannot read the saved board.".to_string())?;
                    let journal = decode(text)?;
                    if let Some(command) = command {
                        let accepted = journal.accept(command, actor)?;
                        if accepted.accepted {
                            cancelled(signal)?;
                            let text = serde_json::to_string(&accepted.journal)
                                .map_err(|e| e.to_string())?;
                            if text.len() > MAX_BYTES {
                                return Err("Board storage limit reached. Export before starting a new board.".into());
                            }
                            settle(store.put(text, KEY)).await.map_err(|_| {
                                "Could not save. Retry the same command ID.".to_string()
                            })?;
                        }
                        Ok(
                            self.stored(
                                accepted.journal,
                                Some(accepted.receipt),
                                accepted.accepted,
                            ),
                        )
                    } else {
                        Ok(self.stored(journal, None, false))
                    }
                });
                let result = settle(operation)
                    .await
                    .map_err(|_| "Storage failed; retry the same command ID.".to_string());
                completion.await?;
                result?
            }
            Self::Server { room } => {
                let body = command
                    .map(|command| serde_json::to_string(&command))
                    .transpose()
                    .map_err(|e| e.to_string())?;
                let remote: Remote = serde_json::from_str(
                    &request(&format!("./api/rooms/{room}"), body, signal).await?,
                )
                .map_err(|_| "Invalid server response.".to_string())?;
                Ok(Stored {
                    journal: decode(Some(remote.record))?,
                    actor: remote.actor,
                    mode: "server",
                    status: "Connected · server storage",
                    receipt: remote.receipt,
                    accepted: remote.accepted.unwrap_or(false),
                })
            }
        }
    }
    pub async fn read(&self) -> Result<Stored, String> {
        self.transact(None, None).await
    }
    pub async fn submit(
        &self,
        command: Command,
        signal: Option<&'static AbortSignal>,
    ) -> Result<Stored, String> {
        self.transact(Some(command), signal).await
    }
    pub fn close(&self) {
        if let Self::Browser { db, .. } = self {
            db.close();
        }
    }
}
