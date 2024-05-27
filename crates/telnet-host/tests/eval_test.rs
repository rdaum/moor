//! Test the eval() built-in to the tune of https://github.com/toddsundsted/stunt/blob/master/test/test_eval.rb

mod common;
use pretty_assertions::assert_eq;

#[test]
#[ignore = "Check currently not implemented"]
fn test_that_eval_cannot_be_called_by_non_programmers() -> eyre::Result<()> {
    common::run_test_as(&["wizard"], |mut client| {
        client.command("; player.programmer = 0;")?;
        assert_eq!(
            "E_PERM: Permission denied.\n",
            client.command(r#"; eval("return 5;");"#)?
        );
        Ok(())
    })
}

#[test]
fn test_that_eval_requires_at_least_one_argument() -> eyre::Result<()> {
    common::run_test_as(&["wizard"], |mut client| {
        assert_eq!("E_ARGS\n", client.command("; return eval();")?);
        Ok(())
    })
}

#[test]
fn test_that_eval_requires_string_arguments() -> eyre::Result<()> {
    common::run_test_as(&["wizard"], |mut client| {
        assert_eq!("E_TYPE\n", client.command("; return eval(1);")?);
        // TODO: uncomment this when eval() accepts multiple arguments
        //assert_eq!("E_TYPE\n", client.command("; return eval(1, 2);")?);
        assert_eq!("E_TYPE\n", client.command("; return eval({});")?);
        Ok(())
    })
}

#[test]
#[ignore = "Multiple args to eval() are not currently implemented"]
fn test_that_eval_evaluates_multiple_strings() -> eyre::Result<()> {
    common::run_test_as(&["wizard"], |mut client| {
        assert_eq!("{1, 15}\n", client.command(r#"; return eval("x = 0;", "for i in [1..5]", "x = x + i;", "endfor", "return x;");"#)?);
        Ok(())
    })
}

#[test]
fn test_that_eval_evaluates_a_single_string() -> eyre::Result<()> {
    common::run_test_as(&["wizard"], |mut client| {
        assert_eq!(
            "{1, 5}\n",
            client.command(r#"; return eval("return 5;");"#)?
        );
        Ok(())
    })
}
