/// RFC-027 D9: a dirty 100,000-line document is megabytes, and twenty of them is a
/// second copy of a meaningful slice of the user's work, not a state directory. Headroom
/// over `DEFAULT_MAX_EDITABLE_BYTES` (the editor's own 4 MiB open cap, `content::open`):
/// a record is the buffer's text plus a small JSON envelope (cursor, viewport, the
/// snapshot it was opened against), so a record of the largest editable document must
/// still fit comfortably under this bound rather than being refused by the escaping
/// overhead alone.
pub const DEFAULT_RECOVERY_MAX_BYTES_PER_RECORD: u64 = 8 * 1024 * 1024;

/// RFC-027 D9: the open set's own bound is twenty documents (RFC-065 D4); twenty records
/// at the per-record bound above is 160 MiB, so this leaves headroom without being large
/// enough to stop meaning anything.
pub const DEFAULT_RECOVERY_MAX_BYTES_TOTAL: u64 = 192 * 1024 * 1024;

/// RFC-027 D9: "a per-buffer byte bound and a total bound" -- two numbers, not the three
/// `TranscriptRetentionLimits` carries (transcripts also bound *per project*; a recovery
/// record's total is already app-wide, since the open set itself, not a per-project
/// policy, is what bounds how many documents from one project could ever be dirty at
/// once).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryRetentionLimits {
    pub max_bytes_per_record: u64,
    pub max_bytes_total: u64,
}

impl RecoveryRetentionLimits {
    pub fn default_limits() -> Self {
        Self {
            max_bytes_per_record: DEFAULT_RECOVERY_MAX_BYTES_PER_RECORD,
            max_bytes_total: DEFAULT_RECOVERY_MAX_BYTES_TOTAL,
        }
    }

    pub fn is_bounded(self) -> bool {
        self.max_bytes_per_record > 0
            && self.max_bytes_total > 0
            && self.max_bytes_per_record <= self.max_bytes_total
    }
}
