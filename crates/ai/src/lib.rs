//! Реестр инструментов ИИ и политика подтверждений.

pub mod tools;

pub use tools::{class_of, policy_for, Confirmation, Context, Policy, ToolClass, TOOLS};
