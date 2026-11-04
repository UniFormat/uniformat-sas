//! Conversion des dates, date-heures et heures SAS (`30` §1 et §4.4).
//!
//! Époque SAS : 1960-01-01. Une date vaut des jours, un datetime des secondes, une heure des secondes depuis minuit.
//! Plage valide : du 1582-01-01 (jour −138 061) au 20 000-12-31 ; hors plage SAS écrit des astérisques et ce
//! module rend [`ConvertError::OutOfRange`], jamais une valeur silencieuse.

/// Échec de conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    /// Valeur hors de la plage que SAS sait afficher (1582-01-01 à l'an 20 000).
    #[error("valeur hors plage")]
    OutOfRange,
    /// Valeur non finie.
    #[error("valeur non finie")]
    NotFinite,
}

/// Jours entre 1960-01-01 et 1970-01-01.
pub const DAYS_1960_TO_1970: i64 = 3653;
/// Secondes entre 1960-01-01T00:00:00 et 1970-01-01T00:00:00.
pub const SECONDS_1960_TO_1970: i64 = 315_619_200;
/// Premier jour affichable : 1582-01-01.
pub const MIN_DAY: i64 = -138_061;

/// Année bissextile selon SAS : grégorienne, sauf les multiples de 4 000 (`30` §4.4).
fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) && year % 4000 != 0
}

/// Jours entre 0001-01-01 et le 1er janvier de `year` dans le calendrier de SAS.
fn days_before_year(year: i64) -> i64 {
    let y = year.saturating_sub(1);
    y.saturating_mul(365)
        .saturating_add(y.div_euclid(4))
        .saturating_sub(y.div_euclid(100))
        .saturating_add(y.div_euclid(400))
        .saturating_sub(y.div_euclid(4000))
}

const MONTH_DAYS: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn year_start_1960() -> i64 {
    days_before_year(1960)
}

/// Dernier jour affichable : 20 000-12-31.
pub fn max_day() -> i64 {
    days_before_year(20_001)
        .saturating_sub(year_start_1960())
        .saturating_sub(1)
}

/// Jour SAS (jours depuis 1960-01-01) → année, mois, jour dans le calendrier de SAS.
///
/// # Errors
/// [`ConvertError::OutOfRange`] hors de 1582-01-01 à 20 000-12-31.
pub fn sas_days_to_ymd(days: i64) -> Result<(i32, u8, u8), ConvertError> {
    if !(MIN_DAY..=max_day()).contains(&days) {
        return Err(ConvertError::OutOfRange);
    }
    let ordinal = days.saturating_add(year_start_1960()); // jours depuis 0001-01-01
    let mut year = ordinal
        .saturating_mul(10_000)
        .div_euclid(3_652_425)
        .saturating_add(1);
    while days_before_year(year) > ordinal {
        year = year.saturating_sub(1);
    }
    while days_before_year(year.saturating_add(1)) <= ordinal {
        year = year.saturating_add(1);
    }
    let mut rest = ordinal.saturating_sub(days_before_year(year));
    for (i, &len) in MONTH_DAYS.iter().enumerate() {
        let len = if i == 1 && is_leap(year) { 29 } else { len };
        if rest < len {
            let month = u8::try_from(i.saturating_add(1)).map_err(|_| ConvertError::OutOfRange)?;
            let day = u8::try_from(rest.saturating_add(1)).map_err(|_| ConvertError::OutOfRange)?;
            return Ok((
                i32::try_from(year).map_err(|_| ConvertError::OutOfRange)?,
                month,
                day,
            ));
        }
        rest = rest.saturating_sub(len);
    }
    Err(ConvertError::OutOfRange)
}

fn finite(v: f64) -> Result<f64, ConvertError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(ConvertError::NotFinite)
    }
}

/// Date SAS → jours depuis 1970-01-01 (`Date32` d'Arrow) : valeur − 3 653 (partie entière, vers −∞).
///
/// Le calcul est celui de la spécification (`30` §1). Au-delà de l'an 4 000 le calendrier de SAS compte un jour
/// de moins par multiple de 4 000 (pas de 29 février) : ce `Date32` désigne alors, dans le calendrier grégorien
/// d'Arrow, un jour qui n'est pas la date que SAS affiche (voir [`sas_days_to_ymd`]).
///
/// # Errors
/// [`ConvertError::NotFinite`] ou [`ConvertError::OutOfRange`] (hors 1582-01-01 à 20 000-12-31).
pub fn sas_date_to_unix_days(v: f64) -> Result<i32, ConvertError> {
    let day = finite(v)?.floor();
    if !(MIN_DAY as f64..=max_day() as f64).contains(&day) {
        return Err(ConvertError::OutOfRange);
    }
    // `day` est entier et dans la plage : conversion exacte.
    let day = day as i64;
    i32::try_from(day.saturating_sub(DAYS_1960_TO_1970)).map_err(|_| ConvertError::OutOfRange)
}

/// Datetime SAS → microsecondes depuis 1970-01-01T00:00:00 (`Timestamp(Microsecond)` d'Arrow), arrondi à la microseconde.
///
/// # Errors
/// [`ConvertError::NotFinite`] ou [`ConvertError::OutOfRange`] (jour hors de 1582-01-01 à 20 000-12-31).
pub fn sas_datetime_to_unix_micros(v: f64) -> Result<i64, ConvertError> {
    let secs = finite(v)?;
    let min = MIN_DAY as f64 * 86_400.0;
    let max = (max_day().saturating_add(1)) as f64 * 86_400.0;
    if secs < min || secs >= max {
        return Err(ConvertError::OutOfRange);
    }
    let micros = ((secs - SECONDS_1960_TO_1970 as f64) * 1e6).round();
    Ok(micros as i64)
}

/// Heure SAS (secondes depuis minuit, négatives et ≥ 86 400 admises) → microsecondes signées.
///
/// Arrow `Time64` ne représente que `0 ≤ t < 86 400 s` : l'appelant choisit `Duration` sinon (`30` §1).
///
/// # Errors
/// [`ConvertError::NotFinite`] ou [`ConvertError::OutOfRange`] (au-delà de ±9·10¹² s, limite de l'`i64`).
pub fn sas_time_to_micros(v: f64) -> Result<i64, ConvertError> {
    let secs = finite(v)?;
    if secs.abs() >= 9.0e12 {
        return Err(ConvertError::OutOfRange);
    }
    Ok((secs * 1e6).round() as i64)
}

/// Composantes d'un datetime SAS, secondes arrondies à l'entier comme `DATETIMEw.`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTimeParts {
    /// Année.
    pub year: i32,
    /// Mois (1 à 12).
    pub month: u8,
    /// Jour (1 à 31).
    pub day: u8,
    /// Heures (0 à 23).
    pub hour: u8,
    /// Minutes (0 à 59).
    pub minute: u8,
    /// Secondes (0 à 59).
    pub second: u8,
}

/// Datetime SAS → composantes dans le calendrier de SAS (secondes arrondies à l'entier le plus proche, moitié vers
/// l'extérieur : `1.5` → `:02`, `0.999` → `:01`).
///
/// # Errors
/// [`ConvertError::NotFinite`] ou [`ConvertError::OutOfRange`].
pub fn sas_datetime_to_parts(v: f64) -> Result<DateTimeParts, ConvertError> {
    let whole = finite(v)?.round();
    let min = MIN_DAY as f64 * 86_400.0;
    let max = (max_day().saturating_add(1)) as f64 * 86_400.0;
    if whole < min || whole >= max {
        return Err(ConvertError::OutOfRange);
    }
    let secs = whole as i64;
    let (year, month, day) = sas_days_to_ymd(secs.div_euclid(86_400))?;
    let tod = secs.rem_euclid(86_400);
    let part = |n: i64| u8::try_from(n).map_err(|_| ConvertError::OutOfRange);
    Ok(DateTimeParts {
        year,
        month,
        day,
        hour: part(tod.div_euclid(3600))?,
        minute: part(tod.rem_euclid(3600).div_euclid(60))?,
        second: part(tod.rem_euclid(60))?,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn epoch_and_neighbours() {
        assert_eq!(sas_days_to_ymd(0), Ok((1960, 1, 1)));
        assert_eq!(sas_days_to_ymd(-1), Ok((1959, 12, 31)));
        assert_eq!(sas_days_to_ymd(59), Ok((1960, 2, 29)));
        assert_eq!(sas_days_to_ymd(60), Ok((1960, 3, 1)));
        assert_eq!(sas_days_to_ymd(3653), Ok((1970, 1, 1)));
        assert_eq!(sas_days_to_ymd(MIN_DAY), Ok((1582, 1, 1)));
        assert_eq!(sas_days_to_ymd(MIN_DAY - 1), Err(ConvertError::OutOfRange));
        assert_eq!(sas_days_to_ymd(2_936_547), Ok((9999, 12, 31)));
    }

    #[test]
    fn calendar_skips_february_29_every_4000_years() {
        // 30 §4.4 : `2 000 000` → 7435-10-26 (au lieu du 25 grégorien), `5 000 000` → 15649-07-17.
        assert_eq!(sas_days_to_ymd(2_000_000), Ok((7435, 10, 26)));
        assert_eq!(sas_days_to_ymd(5_000_000), Ok((15649, 7, 17)));
        assert!(
            !is_leap(4000) && !is_leap(8000) && is_leap(2000) && is_leap(2024) && !is_leap(1900)
        );
    }

    #[test]
    fn last_valid_day_is_year_20000() {
        assert_eq!(sas_days_to_ymd(max_day()), Ok((20000, 12, 31)));
        assert_eq!(
            sas_days_to_ymd(max_day() + 1),
            Err(ConvertError::OutOfRange)
        );
    }

    #[test]
    fn unix_days_and_micros() {
        assert_eq!(sas_date_to_unix_days(3653.0), Ok(0));
        assert_eq!(sas_date_to_unix_days(0.0), Ok(-3653));
        assert_eq!(sas_date_to_unix_days(0.9), Ok(-3653));
        assert_eq!(sas_date_to_unix_days(-0.5), Ok(-3654));
        assert_eq!(
            sas_date_to_unix_days(-200_000.0),
            Err(ConvertError::OutOfRange)
        );
        assert_eq!(
            sas_date_to_unix_days(f64::NAN),
            Err(ConvertError::NotFinite)
        );
        assert_eq!(sas_datetime_to_unix_micros(315_619_200.0), Ok(0));
        assert_eq!(sas_datetime_to_unix_micros(315_619_200.5), Ok(500_000));
        assert_eq!(
            sas_datetime_to_unix_micros(f64::INFINITY),
            Err(ConvertError::NotFinite)
        );
        assert_eq!(
            sas_datetime_to_unix_micros(-1.0e13),
            Err(ConvertError::OutOfRange)
        );
        assert_eq!(sas_time_to_micros(-1.0), Ok(-1_000_000));
        assert_eq!(sas_time_to_micros(90_000.0), Ok(90_000_000_000));
        assert_eq!(sas_time_to_micros(1.0e13), Err(ConvertError::OutOfRange));
    }

    #[test]
    fn datetime_parts_round_seconds() {
        let p = sas_datetime_to_parts(1.5).unwrap();
        assert_eq!(
            (p.year, p.month, p.day, p.hour, p.minute, p.second),
            (1960, 1, 1, 0, 0, 2)
        );
        let p = sas_datetime_to_parts(0.999).unwrap();
        assert_eq!(p.second, 1);
        let p = sas_datetime_to_parts(86_399.6).unwrap();
        assert_eq!(
            (p.year, p.month, p.day, p.hour, p.minute, p.second),
            (1960, 1, 2, 0, 0, 0)
        );
        let p = sas_datetime_to_parts(-1.0).unwrap();
        assert_eq!(
            (p.year, p.month, p.day, p.hour, p.minute, p.second),
            (1959, 12, 31, 23, 59, 59)
        );
    }
}
