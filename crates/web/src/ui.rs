use crate::{render, session::Session};
use js::spawn;
use react::{Dispatch, JSX, SetStateAction, jsx};
use std::rc::Rc;
use webapi::{BlobPart, BlobPropertyBag, blob, document, tags, url, window};
use whiteboard_core::{Command, Operation};
pub struct IconProps {
    pub name: &'static str,
}
pub fn Icon(IconProps { name }: IconProps) -> JSX::Element {
    let content = match name {
        "select" => jsx! {<path d="m5 3 14 10-7 1-3 7Z"/>},
        "eraser" => jsx! {<><path d="m4 12 9-9 8 8-9 9H8l-4-4Z"/><path d="m9 7 8 8M12 20h9"/></>},
        "hand" => {
            jsx! {<path d="M8 12V6a2 2 0 0 1 4 0v6-7a2 2 0 0 1 4 0v7-4a2 2 0 0 1 4 0v8c0 4-3 6-7 6-3 0-5-2-7-5l-3-4a2 2 0 0 1 3-2l2 2"/>}
        }
        "line" => {
            jsx! {<><path d="m5 19 14-14"/><circle cx="5" cy="19" r="1"/><circle cx="19" cy="5" r="1"/></>}
        }
        "rectangle" => jsx! {<rect x="4" y="5" width="16" height="14" rx="2"/>},
        "ellipse" => jsx! {<ellipse cx="12" cy="12" rx="9" ry="7"/>},
        "text" => jsx! {<path d="M5 5h14M12 5v15M8 20h8"/>},
        "undo" => jsx! {<><path d="m8 5-5 5 5 5"/><path d="M3 10h11a6 6 0 0 1 0 12"/></>},
        "redo" => jsx! {<><path d="m16 5 5 5-5 5"/><path d="M21 10H10a6 6 0 0 0 0 12"/></>},
        "replay" => jsx! {<><path d="M4 9a9 9 0 1 1-1 7M4 3v6h6"/><path d="m10 9 5 3-5 3Z"/></>},
        "download" => jsx! {<path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5"/>},
        "clear" => jsx! {<path d="M4 6h16M9 6V3h6v3M7 6l1 15h8l1-15M10 10v7m4-7v7"/>},
        "fit" => jsx! {<path d="M9 3H3v6m12-6h6v6M3 15v6h6m12-6v6h-6"/>},
        "link" => {
            jsx! {<path d="m9 15 6-6M8 16l-1 1a4 4 0 0 1-6-6l5-5a4 4 0 0 1 6 0M16 8l1-1a4 4 0 0 1 6 6l-5 5a4 4 0 0 1-6 0"/>}
        }
        "close" => jsx! {<path d="m6 6 12 12M6 18 18 6"/>},
        _ => jsx! {<><path d="m5 17 2-6L17 1l6 6-10 10-6 2Z"/><path d="m14 4 6 6M5 19l-2 2"/></>},
    };
    jsx! {<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{content}</svg>}
}
pub fn edit(
    session: Rc<Session>,
    operations: Vec<Operation>,
    problem: &'static Dispatch<SetStateAction<Option<String>>>,
    failed: &'static Dispatch<SetStateAction<Option<Command>>>,
) {
    submit(
        session,
        Command {
            command_id: window.crypto().random_uuid(),
            operations,
        },
        problem,
        failed,
    );
}
pub fn submit(
    session: Rc<Session>,
    command: Command,
    problem: &'static Dispatch<SetStateAction<Option<String>>>,
    failed: &'static Dispatch<SetStateAction<Option<Command>>>,
) {
    problem.set(None);
    spawn(Box::new(async move {
        match session.submit(command.clone(), None).await {
            Ok(_) => {
                failed.set(None);
                if let Ok(surface) = session.surface() {
                    surface.clear_draft();
                }
            }
            Err(error) => {
                failed.set(Some(command));
                problem.set(Some(error));
            }
        }
    }));
}
pub fn replay(session: Rc<Session>, problem: &'static Dispatch<SetStateAction<Option<String>>>) {
    spawn(Box::new(async move {
        let result = async {
            session
                .surface()?
                .start_replay(session.journal(), None)
                .await
        }
        .await;
        if let Err(error) = result {
            problem.set(Some(error));
        }
    }));
}
fn download_url(name: &str, address: &str) {
    let anchor = document.create_element(tags::A);
    anchor.set_href(address);
    anchor.set_download(name);
    anchor.click();
}
fn download(name: &str, data: &str, mime: &str) {
    let blob = blob::new(
        Some(&[BlobPart::Str(data)]),
        Some(BlobPropertyBag {
            r#type: Some(mime),
            ..BlobPropertyBag::default()
        }),
    );
    let address = url::create_object_url(blob);
    download_url(name, &address);
    js::set_timeout(
        Box::new(move || url::revoke_object_url(&address)),
        Some(1000.0),
    );
}
pub fn export(
    session: Rc<Session>,
    format: &'static str,
    problem: &'static Dispatch<SetStateAction<Option<String>>>,
) {
    spawn(Box::new(async move {
        let result: Result<(), String> = async {
            match format {
                "json" => download(
                    "whiteboard.json",
                    &serde_json::to_string(&session.journal()).map_err(|e| e.to_string())?,
                    "application/json",
                ),
                "svg" => download(
                    "whiteboard.svg",
                    &render::svg(&session.snapshot())?,
                    "image/svg+xml",
                ),
                "png" => {
                    let capture = render::capture(&session.snapshot(), None, None).await?;
                    download_url("whiteboard.png", &capture.data_url);
                }
                _ => return Err("Unknown export format.".into()),
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            problem.set(Some(error));
        }
    }));
}

pub fn connect(address: &str, room: &str) -> Result<(), String> {
    std::panic::catch_unwind(|| {
        let destination = url::new(address, None);
        if destination.protocol() != "https:"
            && !(destination.protocol() == "http:"
                && (destination.hostname() == "localhost" || destination.hostname() == "127.0.0.1"))
        {
            return Err("Use HTTPS for a remote server.".to_string());
        }
        if room.is_empty()
            || room.len() > 64
            || !room
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return Err("Use letters, numbers, - or _ for the room.".to_string());
        }
        destination.set_hash(&format!("room={room}"));
        window.location().assign(&destination.href());
        Ok(())
    })
    .map_err(|_| "Invalid server URL.".to_string())?
}
