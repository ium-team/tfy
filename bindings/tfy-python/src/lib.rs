use pyo3::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn to_py_json<T: serde::Serialize>(value: T) -> PyResult<String> {
    serde_json::to_string_pretty(&value)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}
fn err(e: anyhow::Error) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(e.to_string())
}

#[pyfunction]
fn index_json(path: String) -> PyResult<String> {
    to_py_json(tfy_core::index_path(PathBuf::from(path)).map_err(err)?)
}

#[pyfunction]
fn expand_json(path: String, scope: String, compactness: Option<String>) -> PyResult<String> {
    to_py_json(
        tfy_core::expand_scope(
            PathBuf::from(path),
            &scope,
            compactness.as_deref().unwrap_or("symbol"),
        )
        .map_err(err)?,
    )
}

#[pyfunction]
fn full_json(path: String, scope: String) -> PyResult<String> {
    to_py_json(tfy_core::full_scope(PathBuf::from(path), &scope).map_err(err)?)
}

#[pyfunction]
fn restore_json(payload_json: String) -> PyResult<String> {
    let payload: tfy_core::RestorePayload = serde_json::from_str(&payload_json)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    to_py_json(tfy_core::restore_payload(payload).map_err(err)?)
}

#[pyfunction]
fn decide_context_json(
    compact_code: String,
    symbols_json: Option<String>,
    diagnostics: Option<String>,
) -> PyResult<String> {
    let symbols: BTreeMap<String, String> = match symbols_json {
        Some(s) if !s.trim().is_empty() => serde_json::from_str(&s)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        _ => BTreeMap::new(),
    };
    to_py_json(tfy_core::decide_context_need(
        &compact_code,
        &symbols,
        diagnostics.as_deref().unwrap_or(""),
    ))
}

#[pyfunction]
fn run_json(
    command: Vec<String>,
    raw_dir: String,
    max_output_bytes: Option<usize>,
) -> PyResult<String> {
    to_py_json(
        tfy_core::run_command(
            &command,
            None,
            PathBuf::from(raw_dir),
            max_output_bytes.unwrap_or(1_000_000),
        )
        .map_err(err)?,
    )
}

#[pyfunction]
fn raw_json(
    raw_dir: String,
    raw_ref: String,
    around: Option<String>,
    context: Option<usize>,
) -> PyResult<String> {
    tfy_core::raw_output(
        PathBuf::from(raw_dir),
        &raw_ref,
        around.as_deref(),
        context.unwrap_or(3),
    )
    .map_err(err)
}

#[pyfunction]
fn languages_json() -> PyResult<String> {
    to_py_json(serde_json::json!({"languages": tfy_core::supported_languages()}))
}

#[pyfunction]
fn eval_code_json(path: String, scope: String, compactness: Option<String>) -> PyResult<String> {
    let exp = tfy_core::expand_scope(
        PathBuf::from(&path),
        &scope,
        compactness.as_deref().unwrap_or("symbol"),
    )
    .map_err(err)?;
    let full = tfy_core::full_scope(PathBuf::from(path), &scope).map_err(err)?;
    to_py_json(serde_json::json!({
        "scope": exp.scope,
        "evaluation": tfy_core::evaluate_code(&full.code, &exp.compact_code),
        "char_metrics": exp.metrics,
    }))
}

#[pymodule]
fn tfy_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(index_json, m)?)?;
    m.add_function(wrap_pyfunction!(expand_json, m)?)?;
    m.add_function(wrap_pyfunction!(full_json, m)?)?;
    m.add_function(wrap_pyfunction!(restore_json, m)?)?;
    m.add_function(wrap_pyfunction!(decide_context_json, m)?)?;
    m.add_function(wrap_pyfunction!(run_json, m)?)?;
    m.add_function(wrap_pyfunction!(raw_json, m)?)?;
    m.add_function(wrap_pyfunction!(languages_json, m)?)?;
    m.add_function(wrap_pyfunction!(eval_code_json, m)?)?;
    Ok(())
}
