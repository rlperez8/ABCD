
function normalizeDate(dateStr) {
        if (!dateStr) return null;

        // Handle "MM-DD-YYYY" → "YYYY-MM-DD"
        if (/^\d{2}-\d{2}-\d{4}$/.test(dateStr)) {
            const [mm, dd, yyyy] = dateStr.split("-");
            return `${yyyy}-${mm}-${dd}`;
        }

        // Handle common string formats like "Mon, 01 Nov 1999 00:00:00 GMT"
        const parsed = new Date(dateStr);
        if (!isNaN(parsed)) {
            return parsed.toISOString().split("T")[0]; // normalize all to "YYYY-MM-DD"
        }

        return null;
}
function findIndexByDate(candles, patternDate) {
    if (!patternDate) return -1;

    const pivotDate = normalizeDate(patternDate);
    if (!pivotDate) return -1;

    return (
        candles.findIndex(obj => {
        const candleDate = normalizeDate(obj.date || obj.candle_date);
        return candleDate === pivotDate;
        }) + 1
    );
    }

export const format_pattern = (candles, pattern, snr_lines, set_chart_data) => {

    let index_A = findIndexByDate(candles, pattern?.pattern_A_pivot_date);
    let index_B = findIndexByDate(candles, pattern?.pattern_B_pivot_date);
    let index_C = findIndexByDate(candles, pattern?.pattern_C_pivot_date);
    let index_D = findIndexByDate(candles, pattern?.trade_entered_date);
    let exit = findIndexByDate(candles, pattern?.trade_exited_date);

    set_chart_data({
        abcd_pattern: {
            ...pattern,
            a: index_A,
            b: index_B,
            c: index_C,
            d: index_D,
            a_price: parseFloat(pattern.pattern_A_high),
            b_price: parseFloat(pattern.pattern_B_low),
            c_price: parseFloat(pattern.pattern_C_high),
            d_price: parseFloat(pattern.trade_entered_price),
            stop_loss: parseFloat(pattern.trade_stop_loss),
            take_profit: parseFloat(pattern.trade_take_profit),
            entered_price: parseFloat(pattern.trade_entered_price),
            exit_price: parseFloat(pattern.trade_exited_price),
            exit_date: exit,
            
        },
        candles: candles,
        snr_lines: snr_lines
    });
}