mod idb;
mod render;
mod session;
mod storage;
mod surface;
mod tools;
mod ui;
mod webmcp;

use crate::render::Viewport;
use crate::session::{Session, SessionInfo};
use crate::surface::{BrushSettings, PaintMode, Surface, Tool};
use crate::ui::Icon;
use js::{settle, spawn};
use react::event::PointerEvent as ReactPointerEvent;
use react::{CSSProperties, JSX, jsx, use_effect, use_ref, use_state};
use std::cell::Cell;
use std::panic::catch_unwind;
use std::rc::Rc;
use webapi::{
    AddEventListenerOptions, Event, EventTargetExt, HTMLCanvasElement, KeyboardEvent, WheelEvent,
    broadcast_channel, document, events, resize_observer, window,
};
use whiteboard_core::{Brush, Clip, Command, Journal, Operation, Point};

const PALETTE: [&str; 6] = [
    "#243b35", "#3d8065", "#4083bf", "#8062ab", "#d97783", "#e4a13c",
];

pub fn App() -> JSX::Element {
    let (snapshot, set_snapshot) = use_state(|| Journal::default().snapshot("initial"));
    let (status, set_status) = use_state(|| "Opening board…".to_string());
    let (saving, set_saving) = use_state(|| 0_u32);
    let (problem, set_problem) = use_state(|| None::<String>);
    let (failed, set_failed) = use_state(|| None::<Command>);
    let (login, set_login) = use_state(|| false);
    let (access_key, set_access_key) = use_state(String::new);
    let (tool, set_tool) = use_state(|| Tool::Pen);
    let (color, set_color) = use_state(|| PALETTE[0].to_string());
    let (width, set_width) = use_state(|| 12.0_f64);
    let (filled, set_filled) = use_state(|| false);
    let (brush_kind, set_brush_kind) = use_state(|| "marker".to_string());
    let (softness, set_softness) = use_state(|| 0.8_f64);
    let (grain, set_grain) = use_state(|| 0.5_f64);
    let (opacity, set_opacity) = use_state(|| 1.0_f64);
    let (paint_mode, set_paint_mode) = use_state(|| PaintMode::Solid);
    let (secondary, set_secondary) = use_state(|| "#ffffff".to_string());
    let (clipping, set_clipping) = use_state(|| None::<Clip>);
    let (view, set_view) = use_state(Viewport::default);
    let (playback, set_playback) = use_state(|| None::<f64>);
    let (panel, set_panel) = use_state(|| None::<&'static str>);
    let (text_at, set_text_at) = use_state(|| None::<Point>);
    let (text, set_text) = use_state(String::new);
    let (server_url, set_server_url) = use_state(String::new);
    let (room_id, set_room_id) = use_state(|| "studio".to_string());
    let (session_info, set_session_info) = use_state(SessionInfo::default);
    let canvas = use_ref(None::<&'static HTMLCanvasElement>);
    let surface = use_ref(None::<Rc<Surface>>);
    let (session, _) = use_state(move || {
        Rc::new(Session::new(
            set_snapshot,
            set_status,
            set_saving,
            set_session_info,
        ))
    });
    let session = Rc::clone(session);

    let mounting = Rc::clone(&session);
    use_effect(
        move || -> Option<Box<dyn FnOnce()>> {
            let mounting = Rc::clone(&mounting);
            let canvas = canvas.current().expect("mounted canvas");
            match Surface::new(
                canvas,
                Viewport::default(),
                set_view,
                set_playback,
                set_problem,
            ) {
                Ok(drawing) => {
                    mounting.attach(Rc::clone(&drawing));
                    surface.set_current(Some(Rc::clone(&drawing)));
                    let resizing = Rc::clone(&drawing);
                    let observer = resize_observer::new(Box::new(move |_, _| resizing.resize()));
                    observer.observe(canvas, None);
                    let zooming = Rc::clone(&drawing);
                    let wheel = webapi::listener(move |event: &WheelEvent| {
                        event.prevent_default();
                        zooming.zoom_at(event.client_x(), event.client_y(), event.delta_y());
                    });
                    let options = AddEventListenerOptions {
                        passive: Some(false),
                        ..AddEventListenerOptions::default()
                    };
                    canvas.add_event_listener(events::Wheel, wheel, Some(&options));
                    let fonts = Rc::clone(&drawing);
                    spawn(Box::new(async move {
                        let _ = settle(document.fonts().ready()).await;
                        fonts.paint();
                    }));
                    let hiding = Rc::clone(&drawing);
                    let hidden = webapi::listener(move |_: &Event| {
                        if document.hidden() {
                            hiding.stop_replay();
                        }
                    });
                    document.add_event_listener_named("visibilitychange", hidden, None);
                    Some(Box::new(move || {
                        observer.disconnect();
                        canvas.remove_event_listener(events::Wheel, wheel, None);
                        document.remove_event_listener_named("visibilitychange", hidden, None);
                        drawing.stop_replay();
                        surface.set_current(None);
                        mounting.detach();
                    }))
                }
                Err(error) => {
                    set_problem.set(Some(error));
                    None
                }
            }
        },
        Some(&()),
    );
    let initializing = Rc::clone(&session);
    use_effect(
        move || -> Option<Box<dyn FnOnce()>> {
            let initializing = Rc::clone(&initializing);
            let active = Rc::new(Cell::new(true));
            let alive = Rc::clone(&active);
            let opening = Rc::clone(&initializing);
            spawn(Box::new(async move {
                if let Err(error) = opening.initialize().await
                    && alive.get()
                {
                    if error == "LOGIN_REQUIRED" {
                        set_status.set("Sign in to join".into());
                        set_login.set(true);
                    } else {
                        set_status.set("Recovery blocked".into());
                        set_problem.set(Some(error));
                    }
                }
            }));
            let incoming = Rc::clone(&initializing);
            let channel = catch_unwind(|| broadcast_channel::new("agent-whiteboard.boards")).ok();
            if let Some(channel) = channel {
                channel.add_event_listener_named(
                    "message",
                    move |_| {
                        let session = Rc::clone(&incoming);
                        if session.ready() && session.mode() == "browser" {
                            spawn(Box::new(async move {
                                let _ = session.refresh().await;
                            }));
                        }
                    },
                    None,
                );
            }
            initializing.set_channel(channel);
            let polling = Rc::clone(&initializing);
            let poll = js::set_interval(
                Box::new(move || {
                    if polling.ready() && polling.mode() == "server" {
                        let session = Rc::clone(&polling);
                        spawn(Box::new(async move {
                            let _ = session.refresh().await;
                        }));
                    }
                }),
                Some(700.0),
            );
            let focused = Rc::clone(&initializing);
            let focus = webapi::listener(move |_: &Event| {
                if focused.ready() {
                    let session = Rc::clone(&focused);
                    spawn(Box::new(async move {
                        let _ = session.refresh().await;
                    }));
                }
            });
            window.add_event_listener(events::Focus, focus, None);
            spawn(Box::new(async move {
                if let Ok(register) = catch_unwind(|| {
                    window
                        .navigator()
                        .service_worker()
                        .register("./sw.js", None)
                }) {
                    let _ = settle(register).await;
                }
            }));
            Some(Box::new(move || {
                active.set(false);
                js::clear_interval(Some(poll));
                window.remove_event_listener(events::Focus, focus, None);
                if let Some(channel) = channel {
                    channel.close();
                }
                initializing.set_channel(None);
                initializing.close();
            }))
        },
        Some(&()),
    );
    use_effect(
        move || {
            if let Some(surface) = surface.current() {
                let ink = match brush_kind.as_str() {
                    "soft" => Brush::Soft {
                        softness: *softness,
                    },
                    "textured" => Brush::Textured {
                        grain: *grain,
                        seed: 42,
                    },
                    _ => Brush::Marker,
                };
                surface.configure(BrushSettings {
                    tool: *tool,
                    color: color.clone(),
                    secondary: secondary.clone(),
                    width: *width,
                    filled: *filled,
                    opacity: *opacity,
                    ink,
                    paint: *paint_mode,
                    clip: clipping.clone(),
                });
            }
        },
        Some(&(
            *tool,
            color.as_str(),
            *width,
            *filled,
            brush_kind.as_str(),
            *softness,
            *grain,
            *opacity,
            *paint_mode,
            secondary.as_str(),
            clipping,
        )),
    );
    let keyboard_session = Rc::clone(&session);
    use_effect(
        move || {
            let keyboard_session = Rc::clone(&keyboard_session);
            let key = webapi::listener(move |event: &KeyboardEvent| {
                if let Some(active) = document.active_element() {
                    let tag = active.tag_name();
                    if tag == "INPUT" || tag == "TEXTAREA" || tag == "SELECT" {
                        return;
                    }
                }
                let key = event.key();
                if (event.meta_key() || event.ctrl_key())
                    && (key.to_lowercase() == "z" || key.to_lowercase() == "y")
                {
                    event.prevent_default();
                    let redo = key.to_lowercase() == "y" || event.shift_key();
                    ui::edit(
                        Rc::clone(&keyboard_session),
                        vec![if redo {
                            Operation::Redo
                        } else {
                            Operation::Undo
                        }],
                        set_problem,
                        set_failed,
                    );
                } else if key == "Delete" || key == "Backspace" {
                    if let Ok(surface) = keyboard_session.surface()
                        && let Some(element) = surface.selected()
                    {
                        event.prevent_default();
                        ui::edit(
                            Rc::clone(&keyboard_session),
                            vec![Operation::Delete {
                                id: element.id,
                                expected_version: element.version,
                            }],
                            set_problem,
                            set_failed,
                        );
                    }
                } else if key == "Escape" {
                    if let Ok(surface) = keyboard_session.surface() {
                        surface.stop_replay();
                        surface.deselect();
                    }
                    set_panel.set(None);
                    set_text_at.set(None);
                }
            });
            window.add_event_listener(events::Keydown, key, None);
            move || window.remove_event_listener(events::Keydown, key, None)
        },
        Some(&()),
    );

    let states = tools::use_tools(Rc::clone(&session), session_info.ready);
    let agent_ready = states.iter().all(|state| state.registered);
    let agent_available = states.iter().any(|state| state.supported)
        && states.iter().all(|state| state.error.is_none());
    let ready = session_info.ready;
    let down = move |event: &ReactPointerEvent<HTMLCanvasElement>| {
        if !ready
            || playback.is_some()
            || failed.is_some()
            || !event.is_primary()
            || event.button() != 0
        {
            return;
        }
        event.prevent_default();
        event.current_target().focus(None);
        event
            .current_target()
            .set_pointer_capture(event.pointer_id());
        if let Some(surface) = surface.current() {
            let pressure = if event.pointer_type() == "pen" {
                event.pressure()
            } else {
                1.0
            };
            if let Some(point) = surface.down(
                event.pointer_id(),
                event.client_x(),
                event.client_y(),
                pressure,
            ) {
                set_text_at.set(Some(point));
                set_text.set(String::new());
            }
        }
    };
    let moved = move |event: &ReactPointerEvent<HTMLCanvasElement>| {
        if let Some(surface) = surface.current() {
            surface.moved(
                event.pointer_id(),
                event.client_x(),
                event.client_y(),
                Some(event.native_event()),
            );
        }
    };
    let releasing = Rc::clone(&session);
    let up = move |event: &ReactPointerEvent<HTMLCanvasElement>| {
        if let Some(surface) = surface.current() {
            let operations = surface.up(
                event.pointer_id(),
                event.client_x(),
                event.client_y(),
                event.native_event(),
            );
            if event
                .current_target()
                .has_pointer_capture(event.pointer_id())
            {
                event
                    .current_target()
                    .release_pointer_capture(event.pointer_id());
            }
            if let Some(operations) = operations {
                ui::edit(Rc::clone(&releasing), operations, set_problem, set_failed);
            }
        }
    };
    let cancel = move || {
        if let Some(surface) = surface.current() {
            surface.cancel_pointer();
        }
    };
    let replaying_session = Rc::clone(&session);
    let undo_session = Rc::clone(&session);
    let redo_session = Rc::clone(&session);
    let retry_session = Rc::clone(&session);
    let clearing = Rc::clone(&session);
    let text_session = Rc::clone(&session);
    let logging_in = Rc::clone(&session);
    jsx! {
        <div className="workspace">
            <header className="topbar">
                <a className="brand" href="./" aria-label="AgentWhiteboard home"><span className="brand-mark"><svg viewBox="0 0 32 32" aria-hidden="true"><path d="M5 23C7 9 11 7 14 18s7 5 12-10" fill="none" stroke="currentColor" strokeWidth="3.5" strokeLinecap="round"/></svg></span><span>{"Agent"}<span className="brand-light">{"Whiteboard"}</span></span></a>
                <span className="header-divider"/><div className="board-title">{"A space for your next idea"}<span>{"HUMAN CURIOSITY. AGENT POSSIBILITY."}</span></div>
                <div className="top-actions"><span className={if session_info.connected{"save-status"}else{"save-status disconnected"}} data-testid="storage-status"><i/>{if *saving>0{"Saving…"}else{status.as_str()}}</span><button className="outline-button connect-button" onClick={move |_|set_panel.set(if *panel==Some("connect"){None}else{Some("connect")})}><Icon name={"link"}/>{if session_info.mode=="server"{"Shared room"}else{"Connect a room"}}</button><button className="dark-button" disabled={!ready} onClick={move |_|set_panel.set(if *panel==Some("export"){None}else{Some("export")})}><Icon name={"download"}/><span>{"Export"}</span></button></div>
            </header>
            <div className="board-menu"><div className="history-controls"><button aria-label="Undo" title="Undo (⌘/Ctrl Z)" disabled={!ready||!snapshot.can_undo||playback.is_some()} onClick={move |_|ui::edit(Rc::clone(&undo_session),vec![Operation::Undo],set_problem,set_failed)}><Icon name={"undo"}/></button><button aria-label="Redo" title="Redo (⌘/Ctrl Shift Z)" disabled={!ready||!snapshot.can_redo||playback.is_some()} onClick={move |_|ui::edit(Rc::clone(&redo_session),vec![Operation::Redo],set_problem,set_failed)}><Icon name={"redo"}/></button><span/><button className={if playback.is_some(){"replay-button playing"}else{"replay-button"}} disabled={!ready||snapshot.sequence==0} onClick={move |_|{if playback.is_some(){if let Ok(surface)=replaying_session.surface(){surface.stop_replay();}}else{ui::replay(Rc::clone(&replaying_session),set_problem);}}}><Icon name={"replay"}/>{if playback.is_some(){"Stop replay"}else{"Replay drawing"}}</button></div><div className="menu-right"><button className="painting-button" onClick={move |_|set_panel.set(if *panel==Some("painting"){None}else{Some("painting")})}>{"Brush & paint"}</button><span className="board-kind">{if session.mode()=="server"{"SHARED CANVAS"}else{"YOUR PERSONAL CANVAS"}}</span><button className="agent-button" onClick={move |_|set_panel.set(if *panel==Some("agent"){None}else{Some("agent")})}><span className={if agent_ready{"agent-dot ready"}else{"agent-dot"}}/><span data-testid="agent-status">{if agent_ready{"Agent ready · 7 tools"}else if agent_available{"Registering tools…"}else{"WebMCP unavailable"}}</span></button></div></div>
            {problem.as_ref().map(|message|jsx!{<div className="error-banner" role="alert"><span>{message}</span>{failed.as_ref().map(|command|{let retry_session=Rc::clone(&retry_session);let command=command.clone();jsx!{<><button onClick={move |_|ui::submit(Rc::clone(&retry_session),command.clone(),set_problem,set_failed)}>{"Retry save"}</button><button onClick={move |_|{set_failed.set(None);if let Some(surface)=surface.current(){surface.clear_draft();}set_problem.set(None);}}>{"Discard draft"}</button></>}})}{failed.is_none().then(||jsx!{<button aria-label="Dismiss error" onClick={move |_|set_problem.set(None)}><Icon name={"close"}/></button>})}</div>})}
            <main className="board-area">
                <canvas className={format!("drawing-canvas tool-{}",tool.class())} aria-label="Whiteboard drawing" role="img" tabIndex={0} ref={canvas} onPointerDown={down} onPointerMove={moved} onPointerUp={up} onPointerCancel={move |_|cancel()} onLostPointerCapture={move |_|cancel()}/>
                {(snapshot.elements.is_empty()&&playback.is_none()).then(||jsx!{<div className="empty-state" aria-hidden="true"><span className="empty-scribble">{"~"}</span><h1>{"Make room for an idea."}</h1><p>{"A line, a little duck, the next big thing."}<br/>{"Pick a pen. Or invite your agent to draw."}</p><span className="empty-hint">{"A BLANK CANVAS. ENDLESS POSSIBILITIES."}</span></div>})}
                <nav className="tool-rail" aria-label="Drawing tools">{[Tool::Select,Tool::Pen,Tool::Eraser,Tool::Line,Tool::Rectangle,Tool::Ellipse,Tool::Text,Tool::Hand].into_iter().map(|name|jsx!{<button key={name.class()} title={name.name()} aria-label={name.name()} aria-pressed={*tool==name} disabled={playback.is_some()} onClick={move |_|{set_tool.set(name);if let Some(surface)=surface.current(){surface.deselect();}}}><Icon name={name.class()}/></button>}).collect::<Vec<_>>()}<span className="rail-separator"/><button aria-label="Clear board" title="Clear board" disabled={snapshot.elements.is_empty()||playback.is_some()} onClick={move |_|set_panel.set(Some("clear"))}><Icon name={"clear"}/></button></nav>
                <section className="ink-tray" aria-label="Marker settings"><div className="colors">{PALETTE.into_iter().map(|value|jsx!{<button key={value} aria-label={format!("Ink {value}")} aria-pressed={color==value} style={CSSProperties::new().set("--ink",value)} onClick={move |_|set_color.set(value.into())}><span/></button>}).collect::<Vec<_>>()}<label className="custom-color" title="Custom ink color"><span>{"+"}</span><input type="color" aria-label="Custom ink color" value={color.clone()} onChange={move|event|set_color.set(event.current_target().value())}/></label></div><span className="tray-divider"/><label className="width-control"><span>{"Stroke"}</span><input aria-label="Stroke width" type="range" min="2" max="80" value={*width} onChange={move|event|set_width.set(event.current_target().value_as_number())}/><span className="width-sample" style={CSSProperties::new().set("--width",format!("{}px",(width/4.0).max(3.0))).set("--ink",color.clone())}/></label>{(*tool==Tool::Rectangle||*tool==Tool::Ellipse).then(||jsx!{<label className="fill-control"><input type="checkbox" checked={*filled} onChange={move|event|set_filled.set(event.current_target().checked())}/>{"Fill"}</label>})}</section>
                <div className="zoom-controls"><button aria-label="Zoom out" onClick={move |_|{if let Some(surface)=surface.current(){let _=surface.set_view(Viewport{zoom:(view.zoom/1.25).max(0.2),..*view});}}}>{"−"}</button><span>{format!("{}%",(view.zoom*100.0).round())}</span><button aria-label="Zoom in" onClick={move |_|{if let Some(surface)=surface.current(){let _=surface.set_view(Viewport{zoom:(view.zoom*1.25).min(8.0),..*view});}}}>{"+"}</button><button aria-label="Fit board" title="Fit board" onClick={move |_|{if let Some(surface)=surface.current(){let _=surface.set_view(Viewport::default());}}}><Icon name={"fit"}/></button></div>
                {playback.map(|progress|jsx!{<div className="playback-hud" role="status"><span className="pulse"/>{"Watching your idea take shape"}<span>{format!("{}%",(progress*100.0).round())}</span><div style={CSSProperties::new().set("width",format!("{}%",progress*100.0))}/></div>})}
                {text_at.as_ref().map(|point|{let point=point.clone();let session=Rc::clone(&text_session);jsx!{<form className="text-editor" onSubmit={move|event|{event.prevent_default();if !text.trim().is_empty(){ui::edit(Rc::clone(&session),vec![Operation::Create{id:window.crypto().random_uuid(),drawing:surface.current().expect("mounted drawing surface").text_drawing(point.clone(),text.clone(),(width*6.0).max(48.0))}],set_problem,set_failed);set_text_at.set(None);}}}><label>{"Add your words"}<textarea aria-label="Board text" autoFocus={true} value={text.clone()} onChange={move|event|set_text.set(event.current_target().value())} placeholder="A thought worth keeping…"/></label><div><button type="button" onClick={move |_|set_text_at.set(None)}>{"Cancel"}</button><button className="dark-button" type="submit">{"Place text"}</button></div></form>}})}
                {(*panel==Some("painting")).then(||jsx!{<section className="popover painting-panel" aria-label="Brush and paint settings"><span className="eyebrow">{"A LITTLE MORE EXPRESSION"}</span><h2>{"Brush & paint"}</h2><label>{"Brush"}<select aria-label="Brush type" value={brush_kind.clone()} onChange={move |event|set_brush_kind.set(event.current_target().value())}><option value="marker">{"Marker"}</option><option value="soft">{"Soft brush"}</option><option value="textured">{"Textured brush"}</option></select></label>{(brush_kind=="soft").then(||jsx!{<label>{"Softness"}<input aria-label="Brush softness" type="range" min="0" max="1" step="0.05" value={*softness} onChange={move |event|set_softness.set(event.current_target().value_as_number())}/></label>})}{(brush_kind=="textured").then(||jsx!{<label>{"Grain"}<input aria-label="Brush grain" type="range" min="0.05" max="1" step="0.05" value={*grain} onChange={move |event|set_grain.set(event.current_target().value_as_number())}/></label>})}<label>{"Opacity"}<input aria-label="Paint opacity" type="range" min="0.05" max="1" step="0.05" value={*opacity} onChange={move |event|set_opacity.set(event.current_target().value_as_number())}/></label><label>{"Paint"}<select aria-label="Paint type" value={match *paint_mode { PaintMode::Solid=>"solid",PaintMode::Linear=>"linear",PaintMode::Radial=>"radial" }} onChange={move |event|set_paint_mode.set(match event.current_target().value().as_str(){"linear"=>PaintMode::Linear,"radial"=>PaintMode::Radial,_=>PaintMode::Solid})}><option value="solid">{"Solid color"}</option><option value="linear">{"Linear gradient"}</option><option value="radial">{"Radial gradient"}</option></select></label>{(*paint_mode!=PaintMode::Solid).then(||jsx!{<label>{"End color"}<input type="color" aria-label="Gradient end color" value={secondary.clone()} onChange={move |event|set_secondary.set(event.current_target().value())}/></label>})}<p>{"Soft and textured brushes apply to pen strokes. Select a path shape to keep new marks inside its outline."}</p><button className="outline-button" onClick={move |_|{if clipping.is_some(){set_clipping.set(None);}else if let Some(surface)=surface.current(){match surface.selected_clip(){Ok(clip)=>{set_clipping.set(Some(clip));set_tool.set(Tool::Pen);},Err(error)=>set_problem.set(Some(error))}}}}>{if clipping.is_some(){"Clear brush clip"}else{"Clip brush to selection"}}</button></section>})}
                {(*panel==Some("export")).then(||{let session=Rc::clone(&session);jsx!{<div className="popover export-menu"><span className="eyebrow">{"TAKE YOUR IDEAS WITH YOU"}</span>{[("png","PNG image","A picture of your board"),("svg","SVG artwork","Crisp, editable vector paths"),("json","Board file","Drawing, history, and undo state")].into_iter().map(|(format,title,description)|{let session=Rc::clone(&session);jsx!{<button key={format} onClick={move |_|{ui::export(Rc::clone(&session),format,set_problem);set_panel.set(None);}}>{title}<span>{description}</span></button>}}).collect::<Vec<_>>()}</div>}})}
                {(*panel==Some("agent")).then(||jsx!{<div className="popover agent-panel"><span className="eyebrow">{"A NEW KIND OF DRAWING PARTNER"}</span><h2>{"Give your agent a pen."}</h2><p>{"Ask a WebMCP-compatible agent:"}</p><blockquote>{"“Draw a little duck on this whiteboard, then replay how you drew it.”"}</blockquote><p>{"Agents have the same canvas: freehand strokes, curves, text, erasing, undo and redo. They can see the result and keep refining it."}</p><small>{if agent_ready{"Native WebMCP tools are registered in this browser."}else{"Use a browser with WebMCP enabled and a compatible agent client. Human drawing works here either way."}}</small></div>})}
                {(*panel==Some("connect")).then(||jsx!{<form className="popover connect-panel" onSubmit={move|event|{event.prevent_default();if let Err(error)=ui::connect(server_url,room_id){set_problem.set(Some(error));}}}><span className="eyebrow">{"ONE CANVAS. MORE POSSIBILITIES."}</span><h2>{"Draw together."}</h2><p>{"Open your hosted whiteboard server to share a canvas across machines."}</p><label>{"Server address"}<input aria-label="Server address" required={true} type="url" placeholder="https://whiteboard.your-tailnet.ts.net" value={server_url.clone()} onChange={move|event|set_server_url.set(event.current_target().value())}/></label><label>{"Room"}<input aria-label="Room name" required={true} value={room_id.clone()} onChange={move|event|set_room_id.set(event.current_target().value())}/></label><button type="submit" className="dark-button">{"Open shared room ↗"}</button></form>})}
                {(*panel==Some("clear")).then(||jsx!{<div className="modal-backdrop"><section className="dialog" role="dialog" aria-modal="true" aria-label="Clear board"><span className="eyebrow">{"A LITTLE ROOM TO BEGIN AGAIN"}</span><h2>{"A fresh canvas?"}</h2><p>{"This clears the visible drawing. You can bring it back with Undo."}</p><div><button onClick={move |_|set_panel.set(None)}>{"Keep drawing"}</button><button className="dark-button" onClick={move |_|{ui::edit(Rc::clone(&clearing),vec![Operation::Clear{expected_sequence:snapshot.sequence}],set_problem,set_failed);set_panel.set(None);}}>{"Clear canvas"}</button></div></section></div>})}
                {(*login).then(||jsx!{<form className="modal-backdrop" onSubmit={move|event|{event.prevent_default();let token=access_key.clone();let session=Rc::clone(&logging_in);spawn(Box::new(async move{let result=async{storage::login(&token).await?;session.initialize().await}.await;match result{Ok(())=>{set_access_key.set(String::new());set_login.set(false);set_problem.set(None);},Err(error)=>set_problem.set(Some(if error=="LOGIN_REQUIRED"{"Incorrect access key.".into()}else{error}))}}));}}><section className="dialog" role="dialog" aria-modal="true" aria-label="Join shared room"><span className="eyebrow">{"YOUR NEXT COLLABORATION"}</span><h2>{"Come draw with us."}</h2><p>{"Enter this server’s access key to join the shared canvas."}</p><label>{"Access key"}<input aria-label="Server access key" autoFocus={true} type="password" required={true} value={access_key.clone()} onChange={move|event|set_access_key.set(event.current_target().value())}/></label><button type="submit" className="dark-button">{"Join room"}</button></section></form>})}
            </main>
            <footer className="statusbar"><span><span className="footer-dot"/>{if session.mode()=="server"{"SHARED ROOM"}else{"LOCAL FIRST"}}<span className="footer-separator">{"/"}</span>{"YOUR IDEAS BELONG TO YOU"}</span><span data-testid="board-status">{format!("{} {}",snapshot.elements.len(),if snapshot.elements.len()==1{"mark"}else{"marks"})}<span className="footer-separator">{"·"}</span><span data-testid="sequence">{format!("Revision {}",snapshot.sequence)}</span><span className="footer-separator">{"·"}</span>{if playback.is_some(){"Replaying history"}else{"All caught up"}}</span><a href="https://github.com/rust-js-lang/agent-whiteboard" target="_blank" rel="noreferrer">{"Follow the build ↗"}</a></footer>
        </div>
    }
}
