export const get_candles = async (symbol) => {

    try{
      const res = await fetch('http://localhost:8000/get_candles', 
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

      return responseData.data
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

  try{
         const res = await fetch("http://localhost:8000/get_support_resistance_lines", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            'symbol':symbol,
          })
      });

      const responseData = await res.json();

      return responseData.data


  } catch (error) {

  }
}
export const fetch_abcd_patterns = async (filters) => {


  const filter = {
    bc_greater: filters.bc_retracement_greater,
    bc_less: filters.bc_retracement_less,
    cd_greater: filters.cd_retracement_greater,
    cd_less: filters.cd_retracement_less
  };

  try {
    const res = await fetch("http://localhost:8080", {
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

    // console.log(data)
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
