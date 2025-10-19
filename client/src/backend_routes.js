export const get_candles = async (symbol) => {

    try{
      const res = await fetch('http://localhost:8000/get_selected_ticker', 
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
      
    //   console.log(responseData)
      return responseData.data
    } catch(error) {
      console.error(error)
    }

}
export const fetch_filtered = async (value) => {

    try {
        const res = await fetch("http://localhost:8000/filtered_patterns", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ value }) 
        });

        const responseData = await res.json();
        console.log(responseData.data)
        // set_ticker_peformance(responseData.data)
        // set_statistics(responseData.stats)
        // set_abcd_patterns(responseData.data)  
        // handle_selected_option(responseData.data, selected_table_option)

        return responseData.data
    
    } catch (error) {
        console.log(error);
    }
}
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
const get_abcd_candles = async () => {
try {
    const res = await fetch("http://localhost:8000/abcd_patterns", {
        method: "GET",
        headers: { "Content-Type": "application/json" }
    });

    const responseData = await res.json();

    // set_statistics(responseData.stats)
    // set_abcd_patterns(responseData.data)
    // set_data(responseData)
    

} catch (error) {
    console.log(error);
}
};