

const Filter = (props) => {

    const {
        filters,
        set_filters,
        fetch_filtered
    } = props

    return(
         <div className='filter_settings'>

            <div className='filter_box'>
                <div className='filter_box_key'>BC Retracement  &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        bc_retracement_greater: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
                
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>BC Retracement  &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        bc_retracement_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
                
            </div>
    
            <div className='filter_box'>
                <div className='filter_box_key'>CD Retracement &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        cd_retracement_greater: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>
            
            <div className='filter_box'>
                <div className='filter_box_key'>CD Retracement &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        cd_retracement_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>AB Leg &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        ab_leg_greater: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>AB Leg &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        ab_leg_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>BC Leg &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        bc_leg_greater: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>BC Leg &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                            bc_leg_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>CD Leg &gt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                        cd_leg_greater: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>CD Leg &lt;</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onChange={(e) => {
                        const updatedFilters = {
                        ...filters,
                            cd_leg_less: e.target.value
                        };

                        set_filters(updatedFilters);  
                        fetch_filtered(updatedFilters);   
                    }}
                    />
                
                </div>
            </div>

            <div className='filter_box'>
                <div className='filter_box_key'>A Date</div>
                <div className='filter_box_val'>
        
                    <input
                    className="input_style"
                    onKeyDown={(e) => {
                            if (e.key === "Enter") {
                            fetch_filtered(filters);   
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
                    onKeyDown={(e) => {
                            if (e.key === "Enter") {
                            fetch_filtered(filters);   
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
                    onKeyDown={(e) => {
                            if (e.key === "Enter") {
                            fetch_filtered(filters);   
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