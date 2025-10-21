import React,{useEffect, useState} from "react";

const Filter = (props) => {

    const {
        filters,
        set_filters,
        fetch_filtered_peformances,
        set_ticker_peformance
    } = props
    
    const [selected_type, set_type] = useState('')
    useEffect(() => {
        if (!filters) return;

        const fetchData = async () => {
            const filtered_data = await fetch_filtered_peformances(filters);
            set_ticker_peformance(filtered_data)

            
        };
        fetchData();
    }, [filters]);

    return(
         <div className='filter_settings'>
            
             <div className='abcd_types_filter_container'>
             <div className={selected_type === 'Standard' ? 'abcd_types_filter_selected' : 'abcd_types_filter'} onClick={()=> {

                const updatedFilters = {
                        ...filters,
                        bc_retracement_greater: 38.2,
                        bc_retracement_less: 61.8,
                        cd_retracement_greater: 100,
                        cd_retracement_less: 127,
                        // ab_leg_greater: e.target.value,
                        // bc_leg_greater: e.target.value,
                        // bc_leg_less: e.target.value,
                        // cd_leg_greater: e.target.value,
                        // cd_leg_less: e.target.value
                        };
                  
                        
                        set_type('Standard')
                        set_filters(updatedFilters);  

                
             }}>Standard</div>
         <div className={selected_type === 'Extended' ? 'abcd_types_filter_selected' : 'abcd_types_filter'} onClick={()=> {

                const updatedFilters = {
                        ...filters,
                        bc_retracement_greater: 61.8,
                        bc_retracement_less: 78.6,
                        cd_retracement_greater: 127,
                        cd_retracement_less: 161.8,
                        // ab_leg_greater: e.target.value,
                        // bc_leg_greater: e.target.value,
                        // bc_leg_less: e.target.value,
                        // cd_leg_greater: e.target.value,
                        // cd_leg_less: e.target.value
                        };
                  
                        set_type('Extended')
                        set_filters(updatedFilters);  

                
             }}>Extended</div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>BC Retracement  &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.bc_retracement_greater}
                 
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        bc_retracement_greater: e.target.value
                        };
                  

                        set_filters(updatedFilters);  
                        
                    }}
                    />
                
                </div>
                
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>BC Retracement  &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.bc_retracement_less}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        bc_retracement_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered_peformances(updatedFilters);   
                    }}
                    />
                
                </div>
                
            </div>
    
            <div className='filter_box'>
                <div className='filter_box_key'>CD Retracement &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.cd_retracement_greater}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        cd_retracement_greater: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered_peformances(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>
            
            <div className='filter_box'>
                <div className='filter_box_key'>CD Retracement &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.cd_retracement_less}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        cd_retracement_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered_peformances(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>AB Leg &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.ab_leg_greater}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        ab_leg_greater: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered_peformances(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>AB Leg &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.ab_leg_less}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        ab_leg_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered_peformances(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>BC Leg &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.bc_leg_greater}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        bc_leg_greater: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered_peformances(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>BC Leg &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.bc_leg_less}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                            bc_leg_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered_peformances(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>CD Leg &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.cd_leg_greater}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        cd_leg_greater: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered_peformances(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>CD Leg &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.cd_leg_less}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                            cd_leg_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered_peformances(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>A Date</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.a_date}
                    onKeyDown={(e) => {
                            if (e.key === "Enter") {
                            fetch_filtered_peformances(filters);   
                            }
                        }}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                            a_date: e.target.value
                        };

                        set_filters(updatedFilters);  
           
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>B Date</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.b_date}
                    onKeyDown={(e) => {
                            if (e.key === "Enter") {
                            fetch_filtered_peformances(filters);   
                            }
                        }}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                            b_date: e.target.value
                        };

                        set_filters(updatedFilters);  
           
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>C Date</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    value={filters.c_date}
                    onKeyDown={(e) => {
                            if (e.key === "Enter") {
                            fetch_filtered_peformances(filters);   
                            }
                        }}
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                            c_date: e.target.value
                        };

                        set_filters(updatedFilters);  
           
                    }}
                    />
                
                </div>
            </div>
                
        </div>
    )
}


export default Filter