mod common;
use pretty_assertions::assert_eq;

#[test]
fn test_echo() -> eyre::Result<()> {
    common::run_test_as(&["wizard"], |mut client| {
        assert_eq!("42\n", client.command("; return 42;")?);
        Ok(())
    })
}

#[test]
fn test_suspend_returns() -> eyre::Result<()> {
    common::run_test_as(&["wizard"], |mut client| {
        client.command("; suspend(0);")?;
        assert_eq!("\"ohai\"\n", client.command("; return \"ohai\";")?);
        Ok(())
    })
}
