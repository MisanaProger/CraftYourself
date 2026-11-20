# calendar

A tiny calendar library used in the Craft Yourself testing course.

- `is_leap_year(year)` — Gregorian leap years.
- `days_in_month(year, month)` — number of days in a month, `None` for an invalid month.

Users report that `days_in_month(2000, 2)` returns 28, but February 2000 had
29 days.
