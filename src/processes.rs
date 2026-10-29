//! Child processes must be reaped even when a request or response is cancelled.
use std::{io, process::{Output, Stdio}, time::Duration};
use tokio::{io::AsyncReadExt, process::{Child, Command}, sync::OwnedSemaphorePermit};

pub struct ManagedChild {
    pub child: Option<Child>,
    permit: Option<OwnedSemaphorePermit>,
}
impl ManagedChild {
    pub fn new(child: Child, permit: Option<OwnedSemaphorePermit>) -> Self {
        Self { child: Some(child), permit }
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let permit = self.permit.take();
            tokio::spawn(async move {
                // kill() also waits/reaps; retain the slot until the child has exited.
                let _ = child.kill().await;
                let _ = child.wait().await;
                drop(permit);
            });
        }
    }
}

/// Bound both runtime and captured output, including a malfunctioning executable.
pub async fn output(command: &mut Command, timeout: Duration, limit: usize) -> io::Result<Output> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let mut process = ManagedChild::new(command.spawn()?, None);
    let child = process.child.as_mut().unwrap();
    let mut stdout = child.stdout.take().unwrap().take(limit as u64 + 1);
    let mut stderr = child.stderr.take().unwrap().take(limit as u64 + 1);
    let result = tokio::time::timeout(timeout, async {
        async fn read_bounded(reader: &mut (impl tokio::io::AsyncRead + Unpin), limit: usize) -> io::Result<Vec<u8>> {
            let mut data=Vec::new();
            reader.read_to_end(&mut data).await?;
            if data.len()>limit { return Err(io::Error::other("Process output limit exceeded")); }
            Ok(data)
        }
        let (out, err)=tokio::try_join!(read_bounded(&mut stdout,limit),read_bounded(&mut stderr,limit))?;
        let status = child.wait().await?;
        Ok(Output { status, stdout: out, stderr: err })
    }).await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Process timed out"))?;
    if result.is_ok() { process.child.take(); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[tokio::test]
    async fn captures_both_pipes_and_bounds_output_and_runtime() {
        let mut cmd = Command::new("sh"); cmd.args(["-c", "printf hello; printf error >&2"]);
        let result = output(&mut cmd, Duration::from_secs(2), 100).await.unwrap();
        assert!(result.status.success()); assert_eq!(result.stdout, b"hello"); assert_eq!(result.stderr, b"error");
        let mut cmd = Command::new("sh"); cmd.args(["-c", "exec yes"]);
        assert!(output(&mut cmd, Duration::from_secs(2), 100).await.unwrap_err().to_string().contains("limit"));
        let mut cmd = Command::new("sleep"); cmd.arg("30");
        assert_eq!(output(&mut cmd, Duration::from_millis(20), 100).await.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_kills_reaps_and_only_then_releases_slot() {
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let child = Command::new("sleep").arg("30").kill_on_drop(true).spawn().unwrap();
        let pid = child.id().unwrap();
        let guard = ManagedChild::new(child, Some(permit));
        assert_eq!(semaphore.available_permits(), 0);
        drop(guard);
        let _permit = tokio::time::timeout(Duration::from_secs(3), semaphore.acquire()).await.unwrap().unwrap();
        // waitpid has reaped it: kill -0 must fail, rather than seeing a zombie.
        assert!(!std::process::Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().unwrap().success());
    }
}
