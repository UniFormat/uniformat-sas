use super::file::{invalid, Member};
use crate::{ByteSource, IoError, Result};

/// Fallible, fused iterator over owned row buffers; decode with `uniformat_sas::xpt::Row`.
pub struct XptRows<'a, S: ByteSource> {
    source: &'a S,
    member: &'a Member,
    next: u64,
    failed: bool,
}
impl<'a, S: ByteSource> XptRows<'a, S> {
    pub(super) fn new(source: &'a S, member: &'a Member) -> Self {
        Self {
            source,
            member,
            next: 0,
            failed: false,
        }
    }
}
impl<S: ByteSource> Iterator for XptRows<'_, S> {
    type Item = Result<Vec<u8>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.next >= self.member.row_count() {
            return None;
        }
        let width = self.member.metadata().row_length();
        let result = self
            .next
            .checked_mul(u64::from(width))
            .and_then(|n| self.member.data_offset().checked_add(n))
            .ok_or_else(|| invalid("XPT row offset overflow"))
            .and_then(|at| {
                self.source
                    .read_range(at, width as usize)
                    .map(|b| b.into_owned())
                    .map_err(IoError::from)
            });
        self.failed = result.is_err();
        self.next = self.next.saturating_add(1);
        Some(result)
    }
}
impl<S: ByteSource> std::iter::FusedIterator for XptRows<'_, S> {}
