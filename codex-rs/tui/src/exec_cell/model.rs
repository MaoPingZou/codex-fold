//! Data model for grouped exec-call history cells in the TUI transcript.
//!
//! An `ExecCell` can represent either a single command or an "exploring" group of related read/
//! list/search commands. The chat widget relies on stable `call_id` matching to route progress and
//! end events into the right cell, and it treats "call id not found" as a real signal (for
//! example, an orphan end that should render as a separate history entry).

use std::time::Duration;
use std::time::Instant;

use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;

#[derive(Debug, Default)]
pub(crate) struct CommandOutput {
    pub(crate) exit_code: i32,
    /// The aggregated stderr + stdout interleaved.
    pub(crate) aggregated_output: String,
}

#[derive(Debug)]
pub(crate) struct ExecCall {
    pub(crate) call_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) parsed: Vec<ParsedCommand>,
    pub(crate) output: Option<CommandOutput>,
    pub(crate) source: ExecCommandSource,
    pub(crate) start_time: Option<Instant>,
    pub(crate) duration: Option<Duration>,
    pub(crate) interaction_input: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ExecCell {
    pub(crate) calls: Vec<ExecCall>,
    animations_enabled: bool,
}

impl ExecCell {
    pub(crate) fn new(call: ExecCall, animations_enabled: bool) -> Self {
        Self {
            calls: vec![call],
            animations_enabled,
        }
    }

    pub(crate) fn add_call(
        &mut self,
        call_id: String,
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
        source: ExecCommandSource,
        interaction_input: Option<String>,
    ) -> bool {
        let call = ExecCall {
            call_id,
            command,
            parsed,
            output: None,
            source,
            start_time: Some(Instant::now()),
            duration: None,
            interaction_input,
        };
        if self.is_groupable_cell() && Self::is_groupable_call(&call) {
            self.calls.push(call);
            true
        } else {
            false
        }
    }

    /// Marks the most recently matching call as finished and returns whether a call was found.
    ///
    /// Callers should treat `false` as a routing mismatch rather than silently ignoring it. The
    /// chat widget uses that signal to avoid attaching an orphan `exec_end` event to an unrelated
    /// active exploring cell, which would incorrectly collapse two transcript entries together.
    pub(crate) fn complete_call(
        &mut self,
        call_id: &str,
        output: CommandOutput,
        duration: Duration,
    ) -> bool {
        let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) else {
            return false;
        };
        call.output = Some(output);
        call.duration = Some(duration);
        call.start_time = None;
        true
    }

    /// Appends an already-finished call (e.g. an interleaved / sub-agent exec end that arrived while
    /// this cell was active) so that consecutive activity aggregates into a single summary instead
    /// of spawning a standalone history entry. Returns `false` when the call is not groupable into
    /// this cell, in which case the caller should fall back to a standalone cell.
    pub(crate) fn push_completed_call(
        &mut self,
        call_id: String,
        command: Vec<String>,
        parsed: Vec<ParsedCommand>,
        source: ExecCommandSource,
        output: CommandOutput,
        duration: Duration,
    ) -> bool {
        let call = ExecCall {
            call_id,
            command,
            parsed,
            output: Some(output),
            source,
            start_time: None,
            duration: Some(duration),
            interaction_input: None,
        };
        if self.is_groupable_cell() && Self::is_groupable_call(&call) {
            self.calls.push(call);
            true
        } else {
            false
        }
    }

    pub(crate) fn should_flush(&self) -> bool {
        // Groupable (agent) cells accumulate consecutive tool calls and are only committed to
        // history by an external flush (e.g. when the assistant starts talking or the turn ends),
        // so they render as a single collapsed summary. User-shell and unified-exec cells keep the
        // legacy behavior of flushing as soon as all their calls complete.
        !self.is_groupable_cell() && self.calls.iter().all(|c| c.duration.is_some())
    }

    pub(crate) fn mark_failed(&mut self) {
        for call in self.calls.iter_mut() {
            if call.duration.is_none() {
                let elapsed = call
                    .start_time
                    .map(|st| st.elapsed())
                    .unwrap_or_else(|| Duration::from_millis(0));
                call.start_time = None;
                call.duration = Some(elapsed);
                call.output
                    .get_or_insert_with(CommandOutput::default)
                    .exit_code = 1;
            }
        }
    }

    /// A cell is "groupable" when every call in it is an agent-issued tool call. Such cells
    /// accumulate consecutive calls (reads, searches, lists and shell commands alike) and render
    /// as a single collapsed count summary instead of one entry per command.
    pub(crate) fn is_groupable_cell(&self) -> bool {
        !self.calls.is_empty() && self.calls.iter().all(Self::is_groupable_call)
    }

    pub(super) fn is_groupable_call(call: &ExecCall) -> bool {
        !matches!(
            call.source,
            ExecCommandSource::UserShell | ExecCommandSource::UnifiedExecInteraction
        )
    }

    pub(crate) fn is_active(&self) -> bool {
        self.calls.iter().any(|c| c.duration.is_none())
    }

    pub(crate) fn animations_enabled(&self) -> bool {
        self.animations_enabled
    }

    pub(crate) fn iter_calls(&self) -> impl Iterator<Item = &ExecCall> {
        self.calls.iter()
    }

    pub(crate) fn append_output(&mut self, call_id: &str, chunk: &str) -> bool {
        if chunk.is_empty() {
            return false;
        }
        let Some(call) = self.calls.iter_mut().rev().find(|c| c.call_id == call_id) else {
            return false;
        };
        let output = call.output.get_or_insert_with(CommandOutput::default);
        output.aggregated_output.push_str(chunk);
        true
    }

    pub(super) fn is_exploring_call(call: &ExecCall) -> bool {
        !matches!(call.source, ExecCommandSource::UserShell)
            && !call.parsed.is_empty()
            && call.parsed.iter().all(|p| {
                matches!(
                    p,
                    ParsedCommand::Read { .. }
                        | ParsedCommand::ListFiles { .. }
                        | ParsedCommand::Search { .. }
                )
            })
    }
}

impl ExecCall {
    pub(crate) fn is_user_shell_command(&self) -> bool {
        matches!(self.source, ExecCommandSource::UserShell)
    }

    pub(crate) fn is_unified_exec_interaction(&self) -> bool {
        matches!(self.source, ExecCommandSource::UnifiedExecInteraction)
    }
}
