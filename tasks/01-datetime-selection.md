# Task 1: Add Time Selection to Date Pickers

**Status: COMPLETED**

## Problem
Currently the date selectors only allow picking dates (YYYY-MM-DD), not times. Users need to be able to select specific times for more granular analytics filtering.

## Milestones

### Backend Changes
- [x] Update `DateRangeQuery` struct to accept datetime strings (ISO 8601 format)
- [x] Update `PaginationQuery` struct to accept datetime strings
- [x] Modify `parse_date_range()` in `src/dashboard/handlers.rs` to parse datetime
- [x] Modify `parse_date_range()` in `src/api/mod.rs` to parse datetime
- [x] Add unit tests for datetime parsing (with and without time component)
- [x] Ensure backwards compatibility with date-only strings
- [x] Handle invalid ranges (start >= end) by swapping values

### Frontend Changes
- [x] Replace `<input type="date">` with `<input type="datetime-local">`
- [x] Update `templates/dashboard/service.html` date inputs
- [x] Update `templates/dashboard/session_list.html` date inputs
- [x] Update `templates/dashboard/location_list.html` date inputs
- [x] Update HTMX includes to send datetime values
- [x] Update JavaScript `updateDateRange()` function in session_list.html
- [x] Ensure URL pattern filter still works with datetime
- [x] Add validation error display when start >= end datetime
- [x] Add red border styling for invalid date inputs

### Testing
- [x] Add E2E test for datetime-local input type verification
- [x] Add E2E test for datetime validation error display
- [x] Verify chart persists after date range change via HTMX

## Technical Notes

### Input Format
Using `<input type="datetime-local">` which provides format: `YYYY-MM-DDTHH:MM`

### Backend Format
Accepts both:
- ISO 8601 datetime: `2024-01-15T14:30`
- Date-only (backwards compatible): `2024-01-15`

When date-only is provided:
- Start dates default to `00:00:00`
- End dates default to `23:59:59`

### Validation
- Frontend: Shows red border and "Start must be before end" / "Invalid range" message
- Backend: Automatically swaps dates if start > end (defensive handling)

## Files Modified
- `src/dashboard/handlers.rs` - Added `parse_datetime_string()`, date range swap logic
- `src/api/mod.rs` - Added `parse_datetime_string()`, date range swap logic, unit tests
- `templates/dashboard/service.html` - datetime-local inputs, validation JS
- `templates/dashboard/session_list.html` - datetime-local inputs, validation JS
- `templates/dashboard/location_list.html` - datetime-local inputs, validation JS
- `e2e/tests/htmx.spec.ts` - Added datetime validation tests
- `e2e/tests/chart-updates.spec.ts` - Updated for datetime-local format
