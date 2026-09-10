//! Local-only captured-source previews. PUBLIC_MODE does not mount these routes.
use axum::{extract::Path, http::{header, StatusCode}, response::{IntoResponse, Response}, routing::{get,post}, Json, Router};
use std::sync::{Arc, Mutex, OnceLock};
use crate::{pipeline::AppState, registry::{Effect, ParamValues}, effects::living_ink::LivingInk};

static RENDERING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct Preview { id: String, bytes: Arc<Vec<u8>>, duration: f64 }
static PREVIEW: OnceLock<Mutex<Option<Preview>>> = OnceLock::new();
fn preview() -> &'static Mutex<Option<Preview>> { PREVIEW.get_or_init(|| Mutex::new(None)) }

fn validate(params: &ParamValues) -> Result<(), String> {
    let defs=LivingInk::default().params();
    for (key,value) in params {
        let def=defs.iter().find(|d| d.id==key).ok_or_else(|| format!("Unknown parameter: {key}"))?;
        if !value.is_finite() || *value<def.min || *value>def.max { return Err(format!("{key} must be between {} and {}",def.min,def.max)); }
    }
    Ok(())
}

async fn render(Json(params): Json<ParamValues>) -> Response {
    if let Err(e)=validate(&params) { return (StatusCode::BAD_REQUEST,e).into_response(); }
    let Ok(permit)=RENDERING.try_lock() else { return (StatusCode::CONFLICT,"A preview is already rendering").into_response(); };
    let Ok(source)=std::env::var("STUDIO_SOURCE") else { return (StatusCode::SERVICE_UNAVAILABLE,"Set STUDIO_SOURCE to a local captured source clip (up to 12 seconds)").into_response(); };
    let work=tokio::task::spawn_blocking(move || -> Result<serde_json::Value,String> {
        if std::fs::metadata(&source).map_err(|e| e.to_string())?.len()>8*1024*1024 { return Err("Source clip exceeds 8 MB".into()); }
        let dir=tempfile::tempdir().map_err(|e|e.to_string())?;
        let out=dir.path().join("preview.ts");
        let preset=dir.path().join("params.json");
        std::fs::write(&preset,serde_json::to_vec(&params).unwrap()).map_err(|e|e.to_string())?;
        let width=std::env::var("STUDIO_WIDTH").unwrap_or_else(|_|"1920".into());
        let args=vec![source,out.display().to_string(),width,"ink".into(),preset.display().to_string()];
        let metrics=crate::offline::render(&args).map_err(|e|e.to_string())?;
        let bytes=std::fs::read(out).map_err(|e|e.to_string())?;
        let id=uuid::Uuid::new_v4().to_string();
        *preview().lock().unwrap()=Some(Preview { id:id.clone(),bytes:Arc::new(bytes),duration:metrics["duration_seconds"].as_f64().unwrap() });
        Ok(serde_json::json!({"url":format!("/api/studio/preview/{id}/index.m3u8"),"metrics":metrics}))
    }).await;
    crate::pipeline::release_idle_memory();
    drop(permit);
    match work { Ok(Ok(v)) => Json(v).into_response(), Ok(Err(e)) => (StatusCode::BAD_REQUEST,e).into_response(), Err(_) => (StatusCode::INTERNAL_SERVER_ERROR,"Preview failed").into_response() }
}
async fn media(Path((id,file)): Path<(String,String)>) -> Response {
    let guard=preview().lock().unwrap();
    let Some(p)=guard.as_ref().filter(|p|p.id==id) else { return StatusCode::NOT_FOUND.into_response(); };
    match file.as_str() {
        "index.m3u8" => ([(header::CONTENT_TYPE,"application/vnd.apple.mpegurl"),(header::CACHE_CONTROL,"no-store")],format!("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:{}\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n#EXTINF:{:.6},\nclip.ts\n#EXT-X-ENDLIST\n",p.duration.ceil(),p.duration)).into_response(),
        "clip.ts" => ([(header::CONTENT_TYPE,"video/mp2t"),(header::CACHE_CONTROL,"private, max-age=3600")],p.bytes.as_ref().clone()).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/studio/preview",post(render)).route("/api/studio/preview/{id}/{file}",get(media))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_known_finite_bounded_parameters() {
        assert!(validate(&ParamValues::from([("trail_strength".into(),0.2)])).is_ok());
        assert!(validate(&ParamValues::from([("trail_strength".into(),2.0)])).is_err());
        assert!(validate(&ParamValues::from([("charcoal".into(),f32::NAN)])).is_err());
        assert!(validate(&ParamValues::from([("bogus".into(),0.2)])).is_err());
    }
}
