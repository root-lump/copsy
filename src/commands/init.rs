use crate::output;
use anyhow::{Result, bail};

pub fn run(shell: &str) -> Result<()> {
    let shell_function = shell_function();
    match shell {
        "zsh" => print!("{shell_function}{}", zsh_completion()),
        "bash" => print!("{shell_function}{}", bash_completion()),
        _ => bail!("Unsupported shell: {shell}. Supported: zsh, bash"),
    }
    Ok(())
}

// Shell function wrapper that captures stdout markers and dispatches them.
// Without markers (e.g. --help), output is passed through unchanged to avoid garbling.
fn shell_function() -> String {
    include_str!("../shell/wrapper.sh")
        .replace("{{MARKER_NAMESPACE}}", output::MARKER_NAMESPACE)
        .replace("{{CD_MARKER}}", output::CD_MARKER)
        .replace("{{LAUNCH_MARKER}}", output::LAUNCH_MARKER)
        .replace("{{OPEN_MARKER}}", output::OPEN_MARKER)
        .replace("{{SETUP_MARKER}}", output::SETUP_MARKER)
        .replace("{{HERDR_LAUNCH_MARKER}}", output::HERDR_LAUNCH_MARKER)
}

fn zsh_completion() -> &'static str {
    include_str!("../shell/completion.zsh")
}

fn bash_completion() -> &'static str {
    include_str!("../shell/completion.bash")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::CommandFactory;

    #[test]
    fn shell_runs_setup_before_cd_and_launch() {
        let shell = shell_function();
        let setup = shell.find("for dir in \"${setup_dirs[@]}\"").unwrap();
        let cd = shell.find("if [[ -n \"$cd_target\" ]]").unwrap();
        let launch = shell.find("for entry in \"${launch_cmds[@]}\"").unwrap();
        assert!(setup < cd);
        assert!(cd < launch);
    }

    #[test]
    fn completions_include_setup_commands_and_flags() {
        for completion in [zsh_completion(), bash_completion()] {
            assert!(completion.contains("config"));
            assert!(completion.contains("setup"));
            assert!(completion.contains("--no-setup"));
        }
    }

    // Guards against drift: completion descriptions are hand-written, so they can
    // silently diverge from the clap `about` text shown by --help.
    #[test]
    fn zsh_subcommand_descriptions_match_clap_about() {
        let zsh = zsh_completion();
        for subcommand in Cli::command().get_subcommands() {
            let name = subcommand.get_name();
            if name == "help" || subcommand.is_hide_set() {
                continue;
            }
            let about = subcommand.get_about().expect("subcommand needs an about");
            assert!(zsh.contains(&format!("'{name}:{about}'")), "{name}");
        }
    }

    #[test]
    fn completions_offer_version_flag() {
        assert!(zsh_completion().contains("{-V,--version}'[Print version]'"));
        assert!(bash_completion().contains("-V --version"));
    }

    #[test]
    fn completions_offer_config_scopes_without_init() {
        for completion in [zsh_completion(), bash_completion()] {
            assert!(completion.contains("repo global"));
            assert!(!completion.contains("config command:(init)"));
            assert!(!completion.contains("compgen -W \"init\""));
        }
    }

    #[test]
    fn shell_uses_every_protocol_marker_from_output_module() {
        let shell = shell_function();
        for marker in output::MARKERS {
            assert!(shell.contains(marker), "missing {marker}");
        }
        assert!(!shell.contains("{{"));
    }

    #[test]
    fn shell_initializes_marker_arrays_for_bash_nounset() {
        let shell = shell_function();
        for array in ["launch_cmds", "open_cmds", "setup_dirs", "herdr_launches"] {
            assert!(shell.contains(&format!("local -a {array}=()")));
            assert!(shell.contains(&format!("if (( ${{#{array}[@]}} )); then")));
        }
    }

    #[test]
    fn completions_dispatch_using_the_detected_subcommand() {
        let zsh = zsh_completion();
        assert!(zsh.contains("case \"$subcommand\" in"));
        assert!(!zsh.contains("case \"${words[1]}\" in"));

        let bash = bash_completion();
        let new = bash.split("        new)").nth(1).unwrap();
        let new = new.split("        add)").next().unwrap();
        let add = bash.split("        add)").nth(1).unwrap();
        let add = add.split("        switch|sw)").next().unwrap();
        assert!(new.contains("--from"));
        assert!(!add.contains("--from"));

        for completion in [zsh, bash] {
            assert!(completion.contains("skip_next=1"));
            assert!(completion.contains("--open)"));
            assert!(completion.contains("--open=*)"));
        }
    }
}
