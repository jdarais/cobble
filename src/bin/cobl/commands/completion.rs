const COMPLETION_SCRIPT_BASH: &'static str = r#"
### cobl command completion ###
#
# Place this script fragment in a location where it will be executed as part of
# shell initialization, such as ~/.bashrc or /etc/bash_completion.d/cobl
#

_cobl_completion() {
    commands="list run clean tool env show"
    case $3 in
        "run" | "clean" | "show" ) COMPREPLY=($($1 list "$2*")) ;;
        "" | "cobl" ) COMPREPLY=($($1 list "$2*") $(compgen -W "$commands" $2)) ;;
        "help" ) COMPREPLY=($(compgen -W "$commands" $2)) ;;
    esac
}

complete -F _cobl_completion cobl

#
### end cobl command completion ###
"#;

pub fn print_completion_script() -> anyhow::Result<()> {
    println!("{}", COMPLETION_SCRIPT_BASH);

    Ok(())
}
