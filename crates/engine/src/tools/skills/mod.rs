//! Skill runtime tools: the bridge that lets the agent load skill bodies
//! on demand.
//!
//! Kept separate from [`crate::skills`] because this module is the
//! *tool-facing* wrapper, while [`crate::skills`] owns the catalog and
//! parsing logic.

pub use use_skill::UseSkillTool;

mod use_skill;
