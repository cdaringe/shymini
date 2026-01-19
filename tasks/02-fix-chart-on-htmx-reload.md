# Task 2: Fix Chart/Graph on HTMX Reload

**Status: COMPLETED**

## Problem
When date range or URL filter changes, HTMX fetches `/service/{id}/stats` which returns HTML partial. The stats cards update correctly, but the chart/graph becomes empty because:
1. The ApexCharts initialization JavaScript only runs on initial page load
2. HTMX replaces the `#stats-container` div which contains the `#chart` element
3. The new `#chart` div has no chart rendered into it

## Milestones

### Diagnose the Issue
- [x] Identify that chart JS is in `service.html` `extra_body` block (runs once on page load)
- [x] Identify that `stats_partial.html` contains chart div but no JS
- [x] Understand HTMX swap lifecycle and `htmx:afterSwap` events

### Fix Implementation
- [x] Move chart data into the stats partial template as a `<script>` block
- [x] Use inline IIFE script that runs immediately when content is swapped
- [x] Ensure chart destroys previous instance before creating new one (prevent memory leaks)
- [x] Pass chart data via Askama template variables in inline JSON
- [x] Test that chart updates correctly on date change
- [x] Test that chart updates correctly on URL filter change

### E2E Tests
- [x] Create `e2e/tests/chart-updates.spec.ts` test file
- [x] Test: Load service detail page, verify chart is visible
- [x] Test: Change date range, wait for HTMX reload, verify chart is still visible
- [x] Test: Change URL filter, wait for HTMX reload, verify chart is still visible
- [x] Test: Verify chart shows empty state gracefully when no matching data

### Code Quality
- [x] Ensure no JavaScript errors in console
- [x] Verify chart renders correctly in all scenarios
- [x] Test with empty data (no hits) - chart should show empty state gracefully

## Technical Approach

### Option 1: Inline Script in Partial (Recommended)
Include chart initialization script directly in `stats_partial.html`:
```html
<!-- stats_partial.html -->
<div id="chart"></div>
<script>
(function() {
    // Destroy existing chart if present
    if (window.shyminiChart) {
        window.shyminiChart.destroy();
    }

    var chartData = { /* ... from template */ };
    var options = { /* ... */ };
    window.shyminiChart = new ApexCharts(document.querySelector("#chart"), options);
    window.shyminiChart.render();
})();
</script>
```

### Option 2: HTMX Event Listener
Keep script in main template but listen for HTMX events:
```javascript
document.body.addEventListener('htmx:afterSwap', function(evt) {
    if (evt.detail.target.id === 'stats-container') {
        // Reinitialize chart
    }
});
```

### Option 3: Separate Chart Endpoint
Create `/service/{id}/chart` endpoint that returns just chart data as JSON, use fetch instead of HTMX for chart updates.

## Files to Modify
- `templates/components/stats_partial.html` - add chart data + initialization
- `templates/dashboard/service.html` - remove or modify chart JS
- `src/dashboard/templates.rs` - may need to add chart data to StatsPartialTemplate
- `e2e/tests/chart-updates.spec.ts` - new test file

## Verification Steps
1. Start server with test data
2. Navigate to service detail page
3. Verify chart renders
4. Change start date
5. Verify chart re-renders with new data
6. Change URL filter
7. Verify chart re-renders with filtered data
8. Run E2E tests
