
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

export const format_pattern = (candles, rust_patterns, snr_lines, set_chart_data) => {

    let index_X = findIndexByDate(candles, rust_patterns?.x_date);
    let index_A = findIndexByDate(candles, rust_patterns?.a_date);
    let index_B = findIndexByDate(candles, rust_patterns?.b_date);
    let index_C = findIndexByDate(candles, rust_patterns?.c_date);
    let index_D = findIndexByDate(candles, rust_patterns?.d_date);
    let exit = findIndexByDate(candles, rust_patterns?.trade_date);

    set_chart_data({
   
        candles: candles,
        snr_lines: snr_lines,
        rust_patterns: {
            
            ...rust_patterns, 
                symbol: rust_patterns.symbol,
                pattern_ABCD_bar_length: rust_patterns.trade_length,
                x: index_X,
                a: index_A,
                b: index_B,
                c: index_C,
                d: index_D,
                x_price: parseFloat(rust_patterns.a_low),
                a_price: parseFloat(rust_patterns.a_high),
                b_price: parseFloat(rust_patterns.b_low),
                c_price: parseFloat(rust_patterns.c_high),
                d_price: parseFloat(rust_patterns.d_low),
                stop_loss: parseFloat(rust_patterns.trade_stop_ltrade_risk_exit_priceoss),
                take_profit: parseFloat(rust_patterns.trade_reward_exit_price),
                entered_price: parseFloat(rust_patterns.d_low),
                exit_price: parseFloat(rust_patterns.trade_current_price),
                exit_date: exit,
            
        }

    });
}