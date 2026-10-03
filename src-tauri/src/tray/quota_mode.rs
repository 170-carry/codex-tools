use crate::models::TrayUsageDisplayMode;

pub(super) fn quota_icon_mode(mode: TrayUsageDisplayMode) -> TrayUsageDisplayMode {
    match mode {
        TrayUsageDisplayMode::FiveHourRemaining | TrayUsageDisplayMode::OneWeekRemaining => mode,
        // The icon always displays remaining quota; hiding text keeps the icon independent.
        _ => TrayUsageDisplayMode::Remaining,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icon_tracks_the_selected_quota_window() {
        for mode in [
            TrayUsageDisplayMode::FiveHourRemaining,
            TrayUsageDisplayMode::OneWeekRemaining,
        ] {
            assert_eq!(quota_icon_mode(mode), mode);
        }
        assert_eq!(
            quota_icon_mode(TrayUsageDisplayMode::Hidden),
            TrayUsageDisplayMode::Remaining
        );
        assert_eq!(
            quota_icon_mode(TrayUsageDisplayMode::Used),
            TrayUsageDisplayMode::Remaining
        );
    }
}
