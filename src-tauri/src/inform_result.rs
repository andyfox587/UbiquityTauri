/// Did set-inform work? One rule for both SSH paths (system ssh + expect on macOS, russh
/// elsewhere), so a silent or failed command is never reported as success.
///
/// UniFi firmware answers a successful set-inform with "Adoption request sent to '<url>'".
/// The command chain falls back through set-inform, mca-cli-op and syswrapper.sh, so the exit
/// status is that of the last attempt.
pub fn interpret(output: &str, exit_status: Option<u32>) -> Result<String, String> {
    let text = output.trim();
    let lower = text.to_lowercase();
    if lower.contains("adoption request sent") {
        return Ok(text.to_string());
    }
    if let Some(code) = exit_status {
        if code != 0 {
            return Err(if text.is_empty() {
                format!("The access point refused the command (exit code {}).", code)
            } else {
                format!("The access point refused the command: {}", text)
            });
        }
    }
    if lower.contains("not found") || lower.contains("error") || lower.contains("denied") || lower.contains("unknown command") {
        return Err(format!("The access point reported a problem: {}", text));
    }
    if exit_status == Some(0) && !text.is_empty() {
        // Finished cleanly with an unfamiliar reply: older firmware words it differently.
        return Ok(text.to_string());
    }
    Err("The access point didn't confirm it will report to VivaSpot. Try again, or call VivaSpot support.".to_string())
}

#[cfg(test)]
mod tests {
    use super::interpret;

    #[test]
    fn adoption_message_is_success() {
        assert!(interpret("Adoption request sent to 'http://unifi.vivaspot.com:8080/inform'.", None).is_ok());
    }
    #[test]
    fn silence_is_not_success() {
        assert!(interpret("", None).is_err());
        assert!(interpret("   ", Some(0)).is_err());
    }
    #[test]
    fn non_zero_exit_is_failure() {
        assert!(interpret("", Some(127)).is_err());
        assert!(interpret("sh: set-inform: not found", Some(127)).is_err());
    }
    #[test]
    fn errors_are_failures() {
        assert!(interpret("error: connection to controller failed", None).is_err());
    }
    #[test]
    fn clean_exit_with_other_reply_is_success() {
        assert!(interpret("inform url set", Some(0)).is_ok());
    }
}
