use super::*;

pub async fn script_eval(
    State(state): State<AppState>,
    Json(req): Json<EvalScriptRequest>,
) -> Result<Json<EvalScriptResponse>, SynapError> {
    let context = ScriptExecContext {
        kv_store: state.kv_store.clone(),
        hash_store: state.hash_store.clone(),
        list_store: state.list_store.clone(),
        set_store: state.set_store.clone(),
        sorted_set_store: state.sorted_set_store.clone(),
    };
    let args = json_args_to_strings(req.args);
    let timeout = req.timeout_ms.map(Duration::from_millis);

    let (result, sha1) = state
        .script_manager
        .eval(context, &req.script, req.keys, args, timeout)
        .await?;

    Ok(Json(EvalScriptResponse { result, sha1 }))
}

pub async fn script_evalsha(
    State(state): State<AppState>,
    Json(req): Json<EvalShaRequest>,
) -> Result<Json<EvalScriptResponse>, SynapError> {
    let context = ScriptExecContext {
        kv_store: state.kv_store.clone(),
        hash_store: state.hash_store.clone(),
        list_store: state.list_store.clone(),
        set_store: state.set_store.clone(),
        sorted_set_store: state.sorted_set_store.clone(),
    };
    let args = json_args_to_strings(req.args);
    let timeout = req.timeout_ms.map(Duration::from_millis);

    let result = state
        .script_manager
        .evalsha(context, &req.sha1, req.keys, args, timeout)
        .await?;

    Ok(Json(EvalScriptResponse {
        result,
        sha1: req.sha1,
    }))
}

pub async fn script_load(
    State(state): State<AppState>,
    Json(req): Json<ScriptLoadRequest>,
) -> Result<Json<ScriptLoadResponse>, SynapError> {
    let sha1 = state.script_manager.load_script(&req.script);
    Ok(Json(ScriptLoadResponse { sha1 }))
}

pub async fn script_exists(
    State(state): State<AppState>,
    Json(req): Json<ScriptExistsRequest>,
) -> Result<Json<ScriptExistsResponse>, SynapError> {
    let exists = state.script_manager.script_exists(&req.hashes);
    Ok(Json(ScriptExistsResponse { exists }))
}

pub async fn script_flush(
    State(state): State<AppState>,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
) -> Result<Json<ScriptFlushResponse>, SynapError> {
    // Block in Hub mode - this would flush ALL users' scripts

    crate::hub::require_standalone_mode(&hub_ctx)?;

    let cleared = state.script_manager.flush();
    Ok(Json(ScriptFlushResponse { cleared }))
}

pub async fn script_kill(
    State(state): State<AppState>,

    crate::hub::HubContextExtractor(hub_ctx): crate::hub::HubContextExtractor,
) -> Result<Json<ScriptKillResponse>, SynapError> {
    // Block in Hub mode - could kill other users' scripts

    crate::hub::require_standalone_mode(&hub_ctx)?;

    let terminated = state.script_manager.kill_running();
    Ok(Json(ScriptKillResponse { terminated }))
}

fn json_args_to_strings(args: Vec<serde_json::Value>) -> Vec<String> {
    args.into_iter()
        .map(|value| match value {
            serde_json::Value::String(s) => s,
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        })
        .collect()
}

fn extract_string_list(
    payload: &serde_json::Value,
    field: &str,
) -> Result<Vec<String>, SynapError> {
    match payload.get(field) {
        Some(serde_json::Value::Array(items)) => Ok(items
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            })
            .collect()),
        Some(_) => Err(SynapError::InvalidRequest(format!(
            "{} must be an array",
            field
        ))),
        None => Ok(Vec::new()),
    }
}

fn extract_json_list(
    payload: &serde_json::Value,
    field: &str,
) -> Result<Vec<serde_json::Value>, SynapError> {
    match payload.get(field) {
        Some(serde_json::Value::Array(items)) => Ok(items.clone()),
        Some(_) => Err(SynapError::InvalidRequest(format!(
            "{} must be an array",
            field
        ))),
        None => Ok(Vec::new()),
    }
}

pub(super) async fn handle_script_eval_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let script = request
        .payload
        .get("script")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'script' field".into()))?;

    let keys = extract_string_list(&request.payload, "keys")?;
    let args = json_args_to_strings(extract_json_list(&request.payload, "args")?);
    let timeout = request
        .payload
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .map(Duration::from_millis);

    let context = ScriptExecContext {
        kv_store: state.kv_store.clone(),
        hash_store: state.hash_store.clone(),
        list_store: state.list_store.clone(),
        set_store: state.set_store.clone(),
        sorted_set_store: state.sorted_set_store.clone(),
    };

    let (result, sha1) = state
        .script_manager
        .eval(context, script, keys, args, timeout)
        .await?;

    Ok(json!({ "result": result, "sha1": sha1 }))
}

pub(super) async fn handle_script_evalsha_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let sha1 = request
        .payload
        .get("sha1")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'sha1' field".into()))?;

    let keys = extract_string_list(&request.payload, "keys")?;
    let args = json_args_to_strings(extract_json_list(&request.payload, "args")?);
    let timeout = request
        .payload
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .map(Duration::from_millis);

    let context = ScriptExecContext {
        kv_store: state.kv_store.clone(),
        hash_store: state.hash_store.clone(),
        list_store: state.list_store.clone(),
        set_store: state.set_store.clone(),
        sorted_set_store: state.sorted_set_store.clone(),
    };

    let result = state
        .script_manager
        .evalsha(context, sha1, keys, args, timeout)
        .await?;

    Ok(json!({ "result": result, "sha1": sha1 }))
}

pub(super) async fn handle_script_load_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let script = request
        .payload
        .get("script")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SynapError::InvalidRequest("Missing 'script' field".into()))?;

    let sha1 = state.script_manager.load_script(script);
    Ok(json!({ "sha1": sha1 }))
}

pub(super) async fn handle_script_exists_cmd(
    state: &AppState,
    request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let hashes = extract_string_list(&request.payload, "hashes")?;
    let exists = state.script_manager.script_exists(&hashes);
    Ok(json!({ "exists": exists }))
}

pub(super) async fn handle_script_flush_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let cleared = state.script_manager.flush();
    Ok(json!({ "cleared": cleared }))
}

pub(super) async fn handle_script_kill_cmd(
    state: &AppState,
    _request: &Request,
) -> Result<serde_json::Value, SynapError> {
    let terminated = state.script_manager.kill_running();
    Ok(json!({ "terminated": terminated }))
}
