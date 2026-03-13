// export const get_candles = async (symbol) => {

//     try{
//       const res = await fetch('http://localhost:8000/get_candles', 
//         {
//           method: "POST", 
//           headers: {"Content-Type": "application/json",}, 
//           body: JSON.stringify({'symbol':symbol})
//         });

//       if (!res.ok) {
//         console.error(`Server Error: ${res.status} - ${res.statusText}`);
//         throw new Error("Request failed");
//       }
//       const responseData = await res.json();

//       console.log('before:',responseData.data[0])

//       return responseData.data
//     } catch(error) {
//       console.error(error)
//     }

// }

export const get_candles = async (symbol) => {

    try{
      const res = await fetch('http://localhost:8080/candles', 
      // const res = await fetch('https://client-server-app.proudsky-e2d2cbaf.centralus.azurecontainerapps.io/candles', 
        {
          method: "POST", 
          headers: {"Content-Type": "application/json",}, 
          body: JSON.stringify({'symbol':symbol})
        });

      if (!res.ok) {
        console.error(`Server Error: ${res.status} - ${res.statusText}`);
        throw new Error("Request failed");
      }
      const responseData = await res.json();

      const renamedCandles = responseData.map(c => ({
        avg_vol: null,
        candle_close: parseFloat(c.close),
        candle_date: new Date(c.date).toUTCString(),
        candle_high: parseFloat(c.high),
        candle_low: parseFloat(c.low),
        candle_open: parseFloat(c.open),
        volume: parseFloat(c.volume),
        symbol: c.symbol
      

      }));


      




      return renamedCandles
    } catch(error) {
      console.error(error)
    }

}


export const fetch_filtered_peformances = async (value) => {

    try {
        const res = await fetch("http://localhost:8000/filtered_peformances", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ value }) 
        });

        const responseData = await res.json();

        return responseData.data
    
    } catch (error) {
        console.log(error);
    }
}
export const fetch_filtered_patterns = async (value) => {

    try {
        const res = await fetch("http://localhost:8000/filtered_patterns", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ value }) 
        });

        const responseData = await res.json();

        return responseData.data
    
    } catch (error) {
        console.log(error);
    }
}
export const get_abcd_candles = async (symbol,filter) => {

  try {
      const res = await fetch("http://localhost:8000/filtered_patterns", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            'symbol':symbol,
            'filter': filter})
      });

      const responseData = await res.json();

      return responseData.data


} catch (error) {
    console.log(error);
}
};
export const get_recent_patterns = async (filters, month) => {

  try {

    const res = await fetch ('http://localhost:8000/recent_patterns', {
      method: "POST",
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({
        month: month,
        bc_retracement_greater: filters.bc_retracement_greater,
        bc_retracement_less: filters.bc_retracement_less,
        cd_retracement_greater: filters.cd_retracement_greater,
        cd_retracement_less: filters.cd_retracement_less,
      })
    })

    
    const responseData = await res.json();
 
    return responseData.data

  }
  catch (error) {
    console.log(error)
  }


}
export const add_watchlist = async (wl_name) => {

  try{
    const res = await fetch ('http://localhost:8000/add_watchlist', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({
        wl_name: wl_name
      })
    })

  }
  catch(error){

  }
}
export const get_all_watchlist = async () => {

  try{
    const res = await fetch ('http://localhost:8000/get_all_watchlist', {
      method: 'GET',
      headers: {'Content-Type': 'application/json'},
  
    })
    const responseData = await res.json()
    return responseData.data
  }
  catch(error){

  }
}
export const delete_watchlist = async (wl_name) => {

  try{
    const res = await fetch ('http://localhost:8000/delete_watchlist', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({
        wl_name: wl_name
      })
  
    })
    const responseData = await res.json()
 
    return responseData.data
  }
  catch(error){

  }
}
export const add_pattern_to_a_watchlist = async (wl_name, pattern_id) => {


  try{
    const res = await fetch ('http://localhost:8000/add_pattern_to_a_watchlist', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({
        wl_name: wl_name,
        pattern_id: pattern_id,
        // change: change,
        // pctChange: pctChange,
        // volume: volume,
        // price: price
      })
  
    })
    const responseData = await res.json()
    return responseData.data
  }
  catch(error){

  }
}
export const get_all_patterns_in_watchlist = async () => {

  try{
    const res = await fetch ('http://localhost:8000/get_all_patterns_in_watchlist', {
      method: 'GET',
      headers: {'Content-Type': 'application/json'},

  
    })
    const responseData = await res.json()

  

    return responseData.data
  }
  catch(error){

  }
}
export const get_monthly_peformance = async (filters, month) => {

  try{
     const res = await fetch ('http://localhost:8000/monthly_peformance', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({
        month: month,
        bc_retracement_greater: filters.bc_retracement_greater,
        bc_retracement_less: filters.bc_retracement_less,
        cd_retracement_greater: filters.cd_retracement_greater,
        cd_retracement_less: filters.cd_retracement_less,
      })

  
    })
    const responseData = await res.json()

    return responseData.data

  } catch(error){

  }
}
export const get_support_resistance_lines = async (symbol) => {

  // try{
  //        const res = await fetch("http://localhost:8000/get_support_resistance_lines", {
  //         method: "POST",
  //         headers: { "Content-Type": "application/json" },
  //         body: JSON.stringify({
  //           'symbol':symbol,
  //         })
  //     });

  //     const responseData = await res.json();

  //     return responseData.data


  // } catch (error) {

  // }
}
export const fetch_abcd_patterns = async (market, filters) => {


  const filter = {
    market: market, 
    bc_greater: filters.bc_retracement_greater,
    bc_less: filters.bc_retracement_less,
    cd_greater: filters.cd_retracement_greater,
    cd_less: filters.cd_retracement_less
  };

  try {
    const res = await fetch("http://localhost:8080/patterns", {
      // const res = await fetch("https://client-server-app.proudsky-e2d2cbaf.centralus.azurecontainerapps.io/patterns", {
    method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
       body: JSON.stringify(filter)
    });

    if (!res.ok) {
      console.error(`Server Error: ${res.status} - ${res.statusText}`);
      throw new Error("Request failed");
    }

    const data = await res.json(); 

    function parsePattern(p) {
      return {
        ...p,

        // prices
        a_close: parseFloat(p.a_close),
        a_high: parseFloat(p.a_high),
        a_low: parseFloat(p.a_low),
        a_open: parseFloat(p.a_open),

        b_close: parseFloat(p.b_close),
        b_high: parseFloat(p.b_high),
        b_low: parseFloat(p.b_low),
        b_open: parseFloat(p.b_open),

        c_close: parseFloat(p.c_close),
        c_high: parseFloat(p.c_high),
        c_low: parseFloat(p.c_low),
        c_open: parseFloat(p.c_open),

        d_close: parseFloat(p.d_close),
        d_high: parseFloat(p.d_high),
        d_low: parseFloat(p.d_low),
        d_open: parseFloat(p.d_open),

        x_close: parseFloat(p.x_close),
        x_high: parseFloat(p.x_high),
        x_low: parseFloat(p.x_low),
        x_open: parseFloat(p.x_open),

        // retracements
        trade_ab_price_retracement: parseFloat(p.trade_ab_price_retracement),
        trade_bc_bar_retracement: parseFloat(p.trade_bc_bar_retracement),
        trade_bc_price_retracement: parseFloat(p.trade_bc_price_retracement),
        trade_cd_bar_retracement: parseFloat(p.trade_cd_bar_retracement),
        trade_cd_bc_price_retracement: parseFloat(p.trade_cd_bc_price_retracement),
        trade_cd_price_retracement: parseFloat(p.trade_cd_price_retracement),
        trade_cd_xa_price_retracement: parseFloat(p.trade_cd_xa_price_retracement),

        // trade prices
        trade_current_price: parseFloat(p.trade_current_price),
        trade_enter_price: parseFloat(p.trade_enter_price),
        trade_reward_exit_price: parseFloat(p.trade_reward_exit_price),
        trade_risk_exit_price: parseFloat(p.trade_risk_exit_price),
        trade_snr: parseFloat(p.trade_snr),

        // pnl
        trade_pnl: parseFloat(p.trade_pnl),

        // lengths
        a_length: parseInt(p.a_length),
        b_length: parseInt(p.b_length),
        c_length: parseInt(p.c_length),
        d_length: parseInt(p.d_length),
        x_length: parseInt(p.x_length),
        trade_length: parseInt(p.trade_length),

        // dates
        // a_date: new Date(p.a_date),
        // b_date: new Date(p.b_date),
        // c_date: new Date(p.c_date),
        // d_date: new Date(p.d_date),
        // x_date: new Date(p.x_date),
        // trade_date: new Date(p.trade_date),

        // timeframe returns
        three_month: parseFloat(p.three_month),
        six_month: parseFloat(p.six_month),
        twelve_month: parseFloat(p.twelve_month),
      };
    }
    data.patterns = data.patterns.map(parsePattern);

    





    return data;

  } catch (e) {
    console.error(e);
  }
};





const get_ab_candles = async () => {
try {
const res = await fetch("http://localhost:8000/ab_candles", {
    method: "GET",
    headers: { "Content-Type": "application/json" }
});

const responseData = await res.json();

const sortedData = responseData.data.sort((a, b) => {
const firstCompare = new Date(a.pattern_A_pivot_date) - new Date(b.pattern_A_pivot_date);

if (firstCompare !== 0) {
    return firstCompare; // sort by first date
}

// if same first date → sort by second date
return new Date(a.pattern_B_pivot_date) - new Date(b.pattern_B_pivot_date);
});


} catch (error) {
console.log(error);
}
};
const get_abc_candles = async () => {
try {
    const res = await fetch("http://localhost:8000/abc_patterns", {
        method: "GET",
        headers: { "Content-Type": "application/json" }
    });

    const responseData = await res.json();
    
    

} catch (error) {
    console.log(error);
}
};
