import React,{useEffect, useState} from "react";

const Filter = (props) => {

    const {
        filters,
        set_filters,
        fetch_filtered_peformances,
        set_ticker_peformance,
        is_loading,
        set_loading,
        set_selected_pattern_index
    } = props
    



    const [selected_type, set_type] = useState('')
    const [scale, set_scale] = useState('')
    useEffect(() => {
        if (!filters) return;

        const fetchData = async () => {
            set_loading(true)
            const filtered_data = await fetch_filtered_peformances(filters);
            set_ticker_peformance(filtered_data)
  
  
            set_loading(false)

            
        };
        fetchData();
    }, [filters]);

    const abcd_types_ = [
         {
            type: 'Standard',
            ratios: {
            bc_retracement_greater: 38.2,
            bc_retracement_less: 61.8,
            cd_retracement_greater: 100,
            cd_retracement_less: 127,
            }
        },
        {
            type: 'Extended',
            ratios: {
            bc_retracement_greater: 38.2,
            bc_retracement_less: 61.8,
            cd_retracement_greater: 127,
            cd_retracement_less: 161.8,
            }
        },
        {
            type: 'Contracted',
            ratios: {
            bc_retracement_greater: 61.8,
            bc_retracement_less: 78.6,
            cd_retracement_greater: 78.6,
            cd_retracement_less: 100,
            }
        },

    ]
    const abcd_scale_types = [
  {
    scale: 'Micro',
    legs: {
      ab_leg_greater: 0,
      ab_leg_less: 9,
      bc_leg_greater: 0,
      bc_leg_less: 9,
      cd_leg_greater: 0,
      cd_leg_less: 9
    }
  },
  {
    scale: 'Tiny',
    legs: {
      ab_leg_greater: 10,
      ab_leg_less: 20,
      bc_leg_greater: 10,
      bc_leg_less: 20,
      cd_leg_greater: 10,
      cd_leg_less: 20
    }
  },
  {
    scale: 'Small',
    legs: {
      ab_leg_greater: 21,
      ab_leg_less: 50,
      bc_leg_greater: 21,
      bc_leg_less: 50,
      cd_leg_greater: 21,
      cd_leg_less: 50
    }
  },
  {
    scale: 'Medium',
    legs: {
      ab_leg_greater: 51,
      ab_leg_less: 200,
      bc_leg_greater: 51,
      bc_leg_less: 200,
      cd_leg_greater: 51,
      cd_leg_less: 200
    }
  },
  {
    scale: 'Large',
    legs: {
      ab_leg_greater: 201,
      ab_leg_less: 1000,
      bc_leg_greater: 201,
      bc_leg_less: 1000,
      cd_leg_greater: 201,
      cd_leg_less: 1000
    }
  }
];



    const abcd_types = [
        'Standard',
        'Extended',
        'Contracted',
        'Bat',
        'Gartley',
        'Butterfly',
        'Crab',
        'Deep Crab',
        'Shark',
        'Cypher',
        '5-0',
        'Three Drives'
        ];

    const handle_ = (selectedType) => {
        // find the pattern definition from your list
        const pattern = abcd_types_.find(p => p.type === selectedType);

        if (!pattern) return; // safety check

        // merge ratios into current filters
        const updatedFilters = {
            ...filters,
            ...pattern.ratios
        };

        // update state
        set_type(selectedType);
        set_filters(updatedFilters);
    };

    function updateLegFilters(selectedScale) {
        const scale = abcd_scale_types.find(s => s.scale === selectedScale);
        if (!scale) return;

        const updatedFilters = {
            ...filters,
            ...scale.legs
        };

        set_scale(selectedScale);
        set_filters(updatedFilters);
        }
    return(

        <div className="margin_container">

            <div className='filter_settings'>

                <div className='table_header_main'>Fitler</div>

                <div className='table_body_main'>

                    <div className="filts">

                        <div className="filter_left">

                            <div className='abcd_types_filter_container'>

                                {abcd_types.map((item,index)=>{
                                    return(
                                        <div 
                                        className={selected_type === item? 'abcd_types_filter_selected' : 'abcd_types_filter'} 
                                        onClick={()=> {handle_(item)}}>
                                            {item}
                                        </div>

                                    )
                                })}

                            </div>

                        </div>

                        <div className="filter_left">

                            <div className='abcd_types_filter_container'>

                                {Object.keys(abcd_scale_types).map((item,index)=>{
                                    return(
                                        <div 
                                        className={scale === abcd_scale_types[item].scale? 'abcd_types_filter_selected' : 'abcd_types_filter'} 
                                        onClick={()=> {updateLegFilters(abcd_scale_types[item].scale)}}>
                                            {abcd_scale_types[item].scale}
                                        </div>

                                    )
                                })}

                            </div>

                        </div>
                    
                        <div className="filter_right">

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

                    </div>

                </div>
                    
            </div>

        </div>
    )
}


export default Filter