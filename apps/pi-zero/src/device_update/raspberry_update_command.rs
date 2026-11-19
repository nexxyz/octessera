use super::UpdateExecutor;
use std::io;
use std::process::{Command, Output};
use std::sync::Arc;

struct CommandUpdateExecutor;

impl UpdateExecutor for CommandUpdateExecutor {
    fn output(&self, command: &mut Command) -> io::Result<Output> {
        command.output()
    }
}

pub(crate) fn production_executor() -> Arc<dyn UpdateExecutor> {
    Arc::new(CommandUpdateExecutor)
}

pub(super) fn update_command(action: &str) -> Command {
    let mut command = Command::new("sudo");
    command.args(["-n", "/usr/local/sbin/octessera-update", action]);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_command_runs_the_sudo_scoped_helper() {
        let command = update_command("apply");
        assert_eq!(
            command
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["-n", "/usr/local/sbin/octessera-update", "apply"]
        );
    }
}
