
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
export const format_pattern = (candles, rust_pattern, snr_lines, set_chart_data) => {

    let index_X = findIndexByDate(candles, rust_pattern?.x_date);
    let index_A = findIndexByDate(candles, rust_pattern?.a_date);
    let index_B = findIndexByDate(candles, rust_pattern?.b_date);
    let index_C = findIndexByDate(candles, rust_pattern?.c_date);
    let index_D = findIndexByDate(candles, rust_pattern?.d_date);
    let exit = findIndexByDate(candles, rust_pattern?.trade_date);

    set_chart_data({
   
        candles: candles,
        snr_lines: snr_lines,
        rust_patterns: {
            
            ...rust_pattern, 
                symbol: rust_pattern.symbol,
                pattern_ABCD_bar_length: rust_pattern.trade_length,
                x: index_X,
                a: index_A,
                b: index_B,
                c: index_C,
                d: index_D,
                x_price: parseFloat(rust_pattern.a_low),
                a_price: parseFloat(rust_pattern.a_high),
                b_price: parseFloat(rust_pattern.b_low),
                c_price: parseFloat(rust_pattern.c_high),
                d_price: parseFloat(rust_pattern.d_low),
                stop_loss: parseFloat(rust_pattern.trade_stop_ltrade_risk_exit_priceoss),
                take_profit: parseFloat(rust_pattern.trade_reward_exit_price),
                entered_price: parseFloat(rust_pattern.d_low),
                exit_price: parseFloat(rust_pattern.trade_current_price),
                exit_date: exit,
            
        }

    });
}

export const format_pattern_ = (candles, rust_pattern) => {

    let index_X = findIndexByDate(candles, rust_pattern?.x_date);
    let index_A = findIndexByDate(candles, rust_pattern?.a_date);
    let index_B = findIndexByDate(candles, rust_pattern?.b_date);
    let index_C = findIndexByDate(candles, rust_pattern?.c_date);
    let index_D = findIndexByDate(candles, rust_pattern?.d_date);
    let exit = findIndexByDate(candles, rust_pattern?.trade_date);

    const pattern = {
        ...rust_pattern,

        symbol: rust_pattern.symbol,
        pattern_ABCD_bar_length: rust_pattern.trade_length,
        x: index_X,
        a: index_A,
        b: index_B,
        c: index_C,
        d: index_D,
        x_price: parseFloat(rust_pattern.a_low),
        a_price: parseFloat(rust_pattern.a_high),
        b_price: parseFloat(rust_pattern.b_low),
        c_price: parseFloat(rust_pattern.c_high),
        d_price: parseFloat(rust_pattern.d_low),
        stop_loss: parseFloat(rust_pattern.trade_stop_loss),
        take_profit: parseFloat(rust_pattern.trade_reward_exit_price),
        entered_price: parseFloat(rust_pattern.d_low),
        exit_price: parseFloat(rust_pattern.trade_current_price),
        exit_date: exit,
    };

    return pattern
}