//! Thin Science Context Protocol adapter for [`lme_rs_agent`].
//!
//! The current official SCP server SDK exposes an MCP-compatible JSON-RPC tool contract.
//! This crate uses that contract while adding experiment context and explicit statistical
//! provenance to the protocol-neutral agent result.

mod mapping;
mod provenance;
mod server;

pub use mapping::{
    ExperimentContext, ExperimentContextInput, FittedModelResource, ScpFitModelRequest,
    ScpFitModelResult,
};
pub use provenance::{
    DatasetProvenance, FitActivity, FitConfiguration, FitOutcome, ModelProvenance,
    SoftwareProvenance,
};
pub use server::{LmeScpServer, ScpAdapterError};
