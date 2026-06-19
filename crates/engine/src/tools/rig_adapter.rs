//! Bridge between mewcode's [`ToolContracts`] trait and Rig's [`ToolDyn`].
//!
//! Rig's agent builder accepts tools that implement `ToolDyn`. Our tools
//! implement mewcode's `ToolContracts` instead (richer descriptors,
//! `ToolError` with hints, `ToolAnnotations`). This adapter wraps any
//! `ToolContracts` implementation so the Rig agent can call it natively
//! during a multi-turn tool-calling loop.
//!
//! The adapter is zero-allocation on the hot path: `definition()` builds
//! a `ToolDefinition` from the cached `ToolDescriptor`, and `call()`
//! delegates to `execute()` then serialises the `ToolOutput` to a JSON
//! string (Rig sends this back to the model as the tool result).

use std::sync::Arc;

use rig_core::completion::ToolDefinition;
use rig_core::tool::ToolDyn;
use rig_core::wasm_compat::WasmBoxedFuture;

use mewcode_protocol::ToolContracts;

/// Wrap a mewcode tool so Rig's agent can call it.
pub struct RigToolAdapter {
    /// The mewcode tool being adapted.
    inner: Arc<dyn ToolContracts>,
    /// Cached descriptor — built once at construction so `definition()`
    /// doesn't re-allocate on every agent turn.
    descriptor: mewcode_protocol::ToolDescriptor,
}

impl RigToolAdapter {
    /// Wrap a mewcode tool for use with Rig's agent builder.
    pub fn new(inner: Arc<dyn ToolContracts>) -> Self {
        let descriptor = inner.descriptor();
        Self { inner, descriptor }
    }
}

impl ToolDyn for RigToolAdapter {
    fn name(&self) -> String {
        self.inner.name().to_string()
    }

    fn definition<'a>(&'a self, _prompt: String) -> WasmBoxedFuture<'a, ToolDefinition> {
        let def = ToolDefinition {
            name: self.descriptor.name.clone(),
            description: self.descriptor.description.clone(),
            parameters: self.descriptor.input_schema.clone(),
        };
        Box::pin(async move { def })
    }

    fn call<'a>(
        &'a self,
        args: String,
    ) -> WasmBoxedFuture<'a, Result<String, rig_core::tool::ToolError>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let input: serde_json::Value =
                serde_json::from_str(&args).unwrap_or(serde_json::Value::Null);
            match inner.execute(input).await {
                Ok(output) => {
                    // Rig expects a string that the provider sends back as
                    // the tool result content. Serialise the ToolOutput's
                    // inner JSON value to a string.
                    Ok(output.0.to_string())
                }
                Err(e) => {
                    // Convert mewcode ToolError into Rig's ToolError. We
                    // serialise the error payload so the model sees the
                    // hint and can retry with corrected input.
                    let payload: mewcode_protocol::ToolErrorPayload = (&e).into();
                    Ok(serde_json::to_string(&payload).unwrap_or_else(|_| {
                        r#"{"error":true,"kind":"other","message":"tool failed"}"#.to_string()
                    }))
                }
            }
        })
    }
}

/// Convert a [`ToolRegistry`](crate::tools::ToolRegistry) into the
/// `Vec<Box<dyn ToolDyn>>` that Rig's agent builder expects.
pub fn rig_tools(registry: &crate::tools::ToolRegistry) -> Vec<Box<dyn ToolDyn>> {
    registry
        .descriptors()
        .iter()
        .map(|d| d.name.as_str())
        .filter_map(|name| registry.get_by_name(name))
        .map(|tool| Box::new(RigToolAdapter::new(tool)) as Box<dyn ToolDyn>)
        .collect()
}
