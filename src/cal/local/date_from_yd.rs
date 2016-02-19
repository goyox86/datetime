use range_check::Check;

use cal::local::Result;
use cal::unit::{Year, Month};
use cal::compound::YearMonthDay;

use super::days_since_epoch::DaysSinceEpoch;
use super::Date;


impl Date {

    /// Creates a new local date instance from the given year and day-of-year
    /// values.
    ///
    /// The values are checked for validity before instantiation, and
    /// passing in values out of range will return an error.
    ///
    /// ### Examples
    ///
    /// Instantiate the 13th of September 2015 based on its year
    /// and day-of-year.
    ///
    /// ```rust
    /// use datetime::cal::DatePiece;
    /// use datetime::cal::local;
    /// use datetime::cal::unit::{Year, Weekday, Month};
    ///
    /// let date = local::Date::yd(2015, 0x100).unwrap();
    /// assert_eq!(date.year(), Year::from(2015));
    /// assert_eq!(date.month(), Month::September);
    /// assert_eq!(date.day(), 13);
    /// ```
    ///
    /// ### Overloading
    ///
    /// If you already have a `Year` value, you can pass it in without having
    /// to dereference it to get the actual year number.
    ///
    /// ```
    /// use datetime::cal::unit::{Year, Month};
    /// use datetime::cal::DatePiece;
    /// use datetime::cal::local;
    ///
    /// let year = Year::from(2015);
    /// let date = local::Date::yd(year, 0x100).unwrap();
    /// assert_eq!(date.year(), year);
    /// ```
    pub fn yd<Y>(year: Y, yearday: i64) -> Result<Date>
    where Y: Into<Year> {
        let year = year.into();

        let days_in_year = if year.is_leap_year() { 367 } else { 366 };
        let yearday = try!(yearday.check_range(0..days_in_year));

        let jan_1 = YearMonthDay { year: year, month: Month::January, day: 0 };
        let mut days = DaysSinceEpoch::from(jan_1);
        days.add(yearday);

        Ok(Date::from(days))
    }
}


#[cfg(test)]
mod unit_test {
    use cal::local;
    use cal::DatePiece;
    use cal::unit::{Year, Month};

    #[test]
    fn day_start_of_year() {
        let date = local::Date::yd(2015, 1).unwrap();
        assert_eq!(Year::from(2015), date.year());
        assert_eq!(Month::January, date.month());
        assert_eq!(1, date.day());
    }
}


#[cfg(test)]
#[allow(trivial_casts)]
mod quickcheck_test {
    use cal::DatePiece;
    use cal::local;

    #[quickcheck]
    fn yeardays(ymd: local::Date) {
        let yd = ymd.yearday();
        let y  = ymd.year();

        let new = local::Date::yd(y, yd as i64).unwrap();
        assert_eq!(ymd, new)
    }
}
