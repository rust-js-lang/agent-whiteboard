//! One native WebMCP surface backed by the same Rust session as human controls.
use js::{Unknown, promise};
use serde::Deserialize;
use serde_json::json;
use std::rc::Rc;
use whiteboard_core::Command;

use crate::render::{self, Region, Viewport};
use crate::session::Session;
use crate::webmcp::{
    self, Annotations, ExecutionOptions, ToolOptions, ToolResponse, ToolState, use_web_mcp,
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ElementQuery {
    offset: Option<usize>,
    limit: Option<usize>,
    sequence: Option<u32>,
    ids: Option<Vec<String>>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CaptureQuery {
    bounds: Option<Region>,
    max_size: Option<u32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Format {
    Json,
    Svg,
    Png,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportQuery {
    format: Format,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum ReplayAction {
    Start,
    Stop,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReplayQuery {
    action: ReplayAction,
    duration_ms: Option<u32>,
}
fn annotations(read_only: bool) -> Annotations {
    Annotations {
        read_only_hint: read_only,
        untrusted_content_hint: read_only,
    }
}
fn check(options: &ExecutionOptions) -> Result<(), String> {
    if options.signal.aborted() {
        Err("Tool call was cancelled.".into())
    } else {
        Ok(())
    }
}
pub fn use_tools(session: Rc<Session>, ready: bool) -> Vec<ToolState> {
    let reading = Rc::clone(&session);
    let state = use_web_mcp(ToolOptions {
        name: "get_board",
        description: "Read the current whiteboard, permissions and limits. Coordinates are logical world units: top-left origin, x right, y down; frame 4096×2304. Draw any subject with strokes, Bezier paths and text. Use get_view to see the actual art.",
        input_schema: webmcp::schema("empty"),
        annotations: annotations(true),
        enabled: ready,
        execute: move |input: Option<&Unknown>, options: ExecutionOptions| {
            let session = Rc::clone(&reading);
            promise(async move {
                let result = async {
                    check(&options)?;
                    let _: Empty = webmcp::input(input)?;
                    session.refresh().await?;
                    let mut value =
                        serde_json::to_value(session.snapshot()).map_err(|e| e.to_string())?;
                    value["boardId"] = json!(
                        webapi::window
                            .location()
                            .hash()
                            .strip_prefix("#room=")
                            .unwrap_or("local")
                    );
                    value["mode"] = json!(session.mode());
                    value["connected"] = json!(session.connected());
                    value["limits"] =
                        json!({"maxOperations":256,"maxPoints":20000,"maxCommandBytes":1048576});
                    Ok(value)
                }
                .await;
                ToolResponse::result(result)
            })
        },
    });
    let querying = Rc::clone(&session);
    let elements = use_web_mcp(ToolOptions {
        name: "get_elements",
        description: "Read editable elements and versions. Geometry kind stroke has points; path has M/L/Q/C/Z segments; text has x,y,text,size. Use returned versions for replace, delete and partial erase. Supply sequence from get_board for consistent pagination; stale pages fail explicitly.",
        input_schema: webmcp::schema("elements"),
        annotations: annotations(true),
        enabled: ready,
        execute: move |input: Option<&Unknown>, options: ExecutionOptions| {
            let session = Rc::clone(&querying);
            promise(async move {
                let result=async{check(&options)?;let query:ElementQuery=webmcp::input(input)?;session.refresh().await?;let snapshot=session.snapshot();if query.sequence.is_some_and(|sequence|sequence!=snapshot.sequence){return Err("Snapshot changed. Restart pagination at the current sequence.".into());}
        let offset=query.offset.unwrap_or(0);let limit=query.limit.unwrap_or(128);if !(1..=256).contains(&limit)||query.ids.as_ref().is_some_and(|ids|ids.len()>256){return Err("Invalid pagination limits.".into());}
        let all=snapshot.elements.into_iter().filter(|element|query.ids.as_ref().is_none_or(|ids|ids.contains(&element.id))).collect::<Vec<_>>();let total=all.len();let elements=all.into_iter().skip(offset).take(limit).collect::<Vec<_>>();let next_offset=(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit));Ok(json!({"sequence":snapshot.sequence,"total":total,"elements":elements,"nextOffset":next_offset}))}.await;
                ToolResponse::result(result)
            })
        },
    });
    let applying = Rc::clone(&session);
    let apply = use_web_mcp(ToolOptions {
        name: "apply_operations",
        description: "Draw and edit freely in one atomic batch. Supply a unique commandId and operations. Create example: {op:'create',id:'bird-wing',drawing:{geometry:{kind:'path',segments:[{op:'M',x:1000,y:1000},{op:'C',cx1:1400,cy1:600,cx2:1700,cy2:1100,x:2100,y:800}]},style:{stroke:'#243b35',width:12,fill:null}}}. Also create stroke points or text, replace/delete with expectedVersion, erase with targets/points/radius, clear with expectedSequence, or standalone undo/redo. Paint stroke/fill accepts #RRGGBB or {kind:'linear',x0,y0,x1,y1,stops:[{offset:0,color:'#ffdd88'},{offset:1,color:'#d85a04'}]}, or {kind:'radial',cx,cy,radius,stops}. Stop opacity is 0–1. Stroke geometry supports style.brush={kind:'soft',softness:0.8} or {kind:'textured',grain:0.5,seed:42}; marker is the default. Drawing clip={segments:[M/L/Q/C/Z...],transform:[1,0,0,1,0,0]} keeps paint inside a local path and moves with the drawing. World coordinates, no scripts or stamps. Identical commandId retries return the original receipt without drawing twice. Returns after durable acceptance and attempted local presentation.",
        input_schema: webmcp::schema("apply"),
        annotations: annotations(false),
        enabled: ready,
        execute: move |input: Option<&Unknown>, options: ExecutionOptions| {
            let session = Rc::clone(&applying);
            promise(async move {
                let result = async {
                    check(&options)?;
                    let command: Command = webmcp::input(input)?;
                    session.submit(command, Some(options.signal)).await
                }
                .await;
                ToolResponse::result(result)
            })
        },
    });
    let viewing = Rc::clone(&session);
    let view = use_web_mcp(ToolOptions {
        name: "get_view",
        description: "SEE the committed drawing as a PNG image plus captured sequence, world bounds and pixel-to-world mapping. Optional bounds={x,y,width,height}; maxSize=64..2048. Inspect the image to improve your drawing. Captures show committed art, not transient replay or drafts.",
        input_schema: webmcp::schema("view"),
        annotations: annotations(true),
        enabled: ready,
        execute: move |input: Option<&Unknown>, options: ExecutionOptions| {
            let session = Rc::clone(&viewing);
            promise(async move {
                let result = async {
                    check(&options)?;
                    let query: CaptureQuery = webmcp::input(input)?;
                    session.refresh().await?;
                    render::capture(&session.snapshot(), query.bounds, query.max_size).await
                }
                .await;
                match result {
                    Ok(image) => ToolResponse::image(&image.info, &image.data_url),
                    Err(error) => ToolResponse::text(error, true),
                }
            })
        },
    });
    let moving = Rc::clone(&session);
    let viewport = use_web_mcp(ToolOptions {
        name: "set_viewport",
        description: "Pan and zoom your local camera without changing artwork, history or document sequence. cx,cy are world coordinates and zoom is 0.2..8. Default center 2048,1152 and zoom 1.",
        input_schema: webmcp::schema("viewport"),
        annotations: annotations(false),
        enabled: ready,
        execute: move |input: Option<&Unknown>, options: ExecutionOptions| {
            let session = Rc::clone(&moving);
            promise(async move {
                let result = (|| {
                    check(&options)?;
                    let view: Viewport = webmcp::input(input)?;
                    session.surface()?.set_view(view)?;
                    Ok(view)
                })();
                ToolResponse::result(result)
            })
        },
    });
    let exporting = Rc::clone(&session);
    let export = use_web_mcp(ToolOptions {
        name: "export_board",
        description: "Return the current editable board JSON, safe SVG artwork or a PNG image. Does not upload or mutate anything. JSON includes accepted drawing history and undo/redo state.",
        input_schema: webmcp::schema("export"),
        annotations: annotations(true),
        enabled: ready,
        execute: move |input: Option<&Unknown>, options: ExecutionOptions| {
            let session = Rc::clone(&exporting);
            promise(async move {
                let result: Result<ToolResponse, String> = async{check(&options)?;let query:ExportQuery=webmcp::input(input)?;session.refresh().await?;match query.format{
            Format::Json=>Ok(ToolResponse::json(&json!({"format":"json","sequence":session.snapshot().sequence,"data":serde_json::to_string(&session.journal()).map_err(|e|e.to_string())?}))),
            Format::Svg=>Ok(ToolResponse::json(&json!({"format":"svg","sequence":session.snapshot().sequence,"data":render::svg(&session.snapshot())?}))),
            Format::Png=>{let capture=render::capture(&session.snapshot(),None,None).await?;Ok(ToolResponse::image(&capture.info,&capture.data_url))}
        }}.await;
                match result {
                    Ok(response) => response,
                    Err(error) => ToolResponse::text(error, true),
                }
            })
        },
    });
    let playing = Rc::clone(&session);
    let replay = use_web_mcp(ToolOptions {
        name: "replay_drawing",
        description: "Replay accepted drawing history from blank white to the final result without changing saved art, undo/redo or sequence. action=start or stop; optional durationMs=500..30000. Live edits cancel replay. Returns when playback finishes or stops.",
        input_schema: webmcp::schema("replay"),
        annotations: annotations(false),
        enabled: ready,
        execute: move |input: Option<&Unknown>, options: ExecutionOptions| {
            let session = Rc::clone(&playing);
            promise(async move {
                let result = async {
                    check(&options)?;
                    let query: ReplayQuery = webmcp::input(input)?;
                    let surface = session.surface()?;
                    match query.action {
                        ReplayAction::Start => {
                            surface
                                .start_replay(session.journal(), query.duration_ms)
                                .await
                        }
                        ReplayAction::Stop => {
                            surface.stop_replay();
                            Ok(crate::surface::PlaybackResult { cancelled: true })
                        }
                    }
                }
                .await;
                ToolResponse::result(result)
            })
        },
    });
    vec![state, elements, apply, view, viewport, export, replay]
}
