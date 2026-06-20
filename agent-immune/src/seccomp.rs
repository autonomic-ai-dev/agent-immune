//! Seccomp-BPF syscall filtering for sandbox subprocesses (Linux only).

#[cfg(target_os = "linux")]
pub fn apply_to_command(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: pre_exec runs in the child between fork and exec; we only install seccomp.
    unsafe {
        cmd.pre_exec(|| install_default_filter().map_err(std::io::Error::other));
    }
}

#[cfg(not(target_os = "linux"))]
pub fn apply_to_command(_cmd: &mut std::process::Command) {}

#[cfg(target_os = "linux")]
fn install_default_filter() -> Result<(), String> {
    use seccompiler::{apply_filter, BpfProgram, SeccompAction, SeccompFilter, TargetArch};
    use std::convert::TryFrom;

    let rules: std::collections::BTreeMap<i64, Vec<seccompiler::SeccompRule>> = ALLOWED_SYSCALLS
        .iter()
        .map(|nr| (*nr, vec![]))
        .collect();

    let arch: TargetArch = std::env::consts::ARCH
        .try_into()
        .map_err(|e| format!("unsupported arch: {e}"))?;

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(1),
        arch,
    )
    .map_err(|e| e.to_string())?;

    let bpf_prog = BpfProgram::try_from(filter).map_err(|e| e.to_string())?;
    apply_filter(&bpf_prog).map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
const ALLOWED_SYSCALLS: &[i64] = &[
    libc::SYS_read,
    libc::SYS_write,
    libc::SYS_open,
    libc::SYS_close,
    libc::SYS_stat,
    libc::SYS_fstat,
    libc::SYS_lseek,
    libc::SYS_mmap,
    libc::SYS_mprotect,
    libc::SYS_munmap,
    libc::SYS_brk,
    libc::SYS_rt_sigaction,
    libc::SYS_rt_sigprocmask,
    libc::SYS_rt_sigreturn,
    libc::SYS_ioctl,
    libc::SYS_access,
    libc::SYS_pipe,
    libc::SYS_dup,
    libc::SYS_dup2,
    libc::SYS_nanosleep,
    libc::SYS_getpid,
    libc::SYS_clone,
    libc::SYS_execve,
    libc::SYS_exit,
    libc::SYS_wait4,
    libc::SYS_uname,
    libc::SYS_fcntl,
    libc::SYS_getcwd,
    libc::SYS_chdir,
    libc::SYS_readlink,
    libc::SYS_gettimeofday,
    libc::SYS_getuid,
    libc::SYS_getgid,
    libc::SYS_geteuid,
    libc::SYS_getegid,
    libc::SYS_arch_prctl,
    libc::SYS_set_tid_address,
    libc::SYS_set_robust_list,
    libc::SYS_futex,
    libc::SYS_clock_gettime,
    libc::SYS_exit_group,
    libc::SYS_openat,
    libc::SYS_newfstatat,
    libc::SYS_prlimit64,
    libc::SYS_getrandom,
];
