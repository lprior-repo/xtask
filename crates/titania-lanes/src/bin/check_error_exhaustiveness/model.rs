use std::{io::ErrorKind, path::PathBuf};

use titania_core::TargetProject;

#[derive(Clone, Copy)]
pub(super) struct TargetRelativePath {
    value: &'static str,
}

impl TargetRelativePath {
    const fn new(value: &'static str) -> Self {
        Self { value }
    }

    pub(super) const fn as_str(self) -> &'static str {
        self.value
    }

    pub(super) fn in_target(self, target: &TargetProject) -> PathBuf {
        target.as_std_path().join(self.value)
    }
}

pub(super) struct Oracle {
    pub(super) path: TargetRelativePath,
    pub(super) function: &'static str,
}

pub(super) struct Check {
    pub(super) type_name: &'static str,
    pub(super) enum_path: TargetRelativePath,
    pub(super) domain_label: &'static str,
    pub(super) oracles: &'static [Oracle],
}

pub(super) enum DomainFile {
    Present(String),
    Absent,
    Unreadable(ErrorKind),
}

const JOURNAL_ORACLES: &[Oracle] = &[
    Oracle {
        path: TargetRelativePath::new("fuzz/src/lib.rs"),
        function: "assert_typed_journal_error",
    },
    Oracle {
        path: TargetRelativePath::new("fuzz/fuzz_targets/decode_record.rs"),
        function: "assert_typed_journal_error",
    },
    Oracle {
        path: TargetRelativePath::new("fuzz/fuzz_targets/journal_decode.rs"),
        function: "assert_typed_journal_error",
    },
    Oracle {
        path: TargetRelativePath::new("fuzz/tests/proptest_journal_error_exhaustiveness.rs"),
        function: "assert_known_journal_error",
    },
];

const IPC_ORACLES: &[Oracle] = &[Oracle {
    path: TargetRelativePath::new("fuzz/src/lib.rs"),
    function: "assert_typed_ipc_error",
}];

const VALIDATION_ORACLES: &[Oracle] = &[Oracle {
    path: TargetRelativePath::new("fuzz/src/lib.rs"),
    function: "assert_typed_validation_error",
}];

pub(super) const CHECKS: &[Check] = &[
    Check {
        type_name: "JournalError",
        enum_path: TargetRelativePath::new("crates/vb_storage/src/error/mod.rs"),
        domain_label: "vb_storage",
        oracles: JOURNAL_ORACLES,
    },
    Check {
        type_name: "IpcError",
        enum_path: TargetRelativePath::new("crates/vb_ipc/src/error.rs"),
        domain_label: "vb_ipc",
        oracles: IPC_ORACLES,
    },
    Check {
        type_name: "ValidationError",
        enum_path: TargetRelativePath::new("crates/vb_validate/src/lib.rs"),
        domain_label: "vb_validate",
        oracles: VALIDATION_ORACLES,
    },
];
