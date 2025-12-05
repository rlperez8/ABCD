import React, {useState} from "react"
import dropup from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/arrow_up.png';
import dropdown from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/dropdown.png';
import InfiniteTable from "./InfiniteTable";

const Section = (props) => {

     const {
        title,
        body,
        length,
        set_market,
        market,
        sort_by_result,
        table_result,
        selected_harmonic_pattern,
        set_selected_harmonic_pattern,
        harmonic_patterns,
        update_harmonic_pattern,
        price_retracement,
        activeFilters,
        setActiveFilters
    } = props
   
    const [is_collapse, set_collapse] = useState(false)

    const get_css = () => {

        if(title === 'Build Pattern'){
            return 'bp-section'
        }
        if(title === 'Recent Patterns'){
            return 'rp-section'
        }

    }
    const activateMarket = value => {
        setActiveFilters(prev => ({
            ...prev,
            market: { active: true, filter: value }
        }));
    };
    const activateResult = value => {
        setActiveFilters(prev => ({
            ...prev,
            result: { active: true, filter: value }
        }));
    };
    const activateRetracement = value => {

        setActiveFilters(prev => ({
            ...prev,
            retracement: { 
                active: true, 
                filter: value 
            }
        }));
    };
    

    return(
        <div className={is_collapse ? 'patterns_table_main' : get_css() }>
            
            <div className="margin-">
        
                <div className='table_header_main'>
                    <div className='table_header_text'>{title}</div>
                    
                    <div className='table_header_count'>{length}</div>
                    <div className='settings_icon' onClick={()=>{set_collapse(!is_collapse)}}>
                    <img className='icon_img' src={is_collapse ? dropdown : dropup}/>
                    </div>
                </div>

                <div className='patterns_settings'>

                    <div className="market-wrapper">
                        <div className={activeFilters.market.filter === 'Bullish' ? "market-selected" : 'market-'} onClick={() => activateMarket("Bullish")}>Bullish</div>
                        <div className={activeFilters.market.filter === 'Bearish' ? "market-selected" : 'market-'} onClick={() => activateMarket("Bearish")}>Bearish</div>

                    </div>

                    <div className="market-wrapper">
                        <div className={activeFilters.result.filter  === 'true' ? "market-selected" : 'market-'}  onClick={() =>{activateResult("true")}}>Won</div>
                        <div className={activeFilters.result.filter  === 'false' ? "market-selected" : 'market-'}  onClick={()=>{activateResult("false")}}>Lost</div>
                        <div className={activeFilters.result.filter  === 'Open' ? "market-selected" : 'market-'}  onClick={()=>{activateResult("Open")}}>Open</div>

                    </div>

                    
                    
                </div>

                <div className='patterns_settings_3'>

                    <div className='patterns_settings_2'>

                        <div className="retracement-header">Price</div>

                        <div className="retracement-body">
                            <div className='patterns_settings_left'>

                                {Object.keys(harmonic_patterns).map((key, index) => {

                                    let filter = harmonic_patterns[key]
                                    const isSelected = activeFilters.retracement.filter?.type === harmonic_patterns[key].type;

                                    return (
                                        <div
                                            key={key}
                                            className={isSelected ? "selected-pattern-type" : "pattern_type"}
                                            onClick={() => activateRetracement(filter)}
                                        >
                                            {harmonic_patterns[key].type}
                                        </div>
                                    );
                                })}
             
                        </div>

                        <div className='patterns_settings_right'>

                            <div className='filter_box'>
                                    <div className='filter_box_key'>BX  &gt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                            className="input_style"
                                            value={activeFilters.retracement.filter?.ratios.ab_xa_gr}
                                
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>BX  &lt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                            className="input_style"
                                            value={activeFilters.retracement.filter?.ratios.ab_xa_lt}
                                    
        
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>BC  &lt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                            className="input_style"
                                            value={activeFilters.retracement.filter?.ratios.bc_ab_gr}
                                    
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>BC  &gt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                        className="input_style"
                                        value={activeFilters.retracement.filter?.ratios.bc_ab_lt}
                                    

                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>CD  &gt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                        className="input_style"
                                        value={activeFilters.retracement.filter?.ratios.cd_bc_gr}
                                    
                                
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>CD  &lt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                        className="input_style"
                                        value={activeFilters.retracement.filter?.ratios.cd_bc_lt}
                                    
                                      
                                        />
                                    
                                    </div>
                                    
                            </div>

                            <div className='filter_box'>
                                    <div className='filter_box_key'>DX  &gt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                        className="input_style"
                                        value={activeFilters.retracement.filter?.ratios.cd_xa_gr}
                                    
                                
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>DX  &lt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                        className="input_style"
                                        value={activeFilters.retracement.filter?.ratios.cd_xa_lt}
                                    
                                      
                                        />
                                    
                                    </div>
                                    
                            </div>


                        </div>

                        </div>

                    </div>

                    <div className='patterns_settings_2'>

                        <div className="retracement-header">Length</div>
                        <div className="retracement-body">
                            <div className='patterns_settings_left'>
                            <div className="pattern_type">Standard</div>
                            <div className="pattern_type">Extended</div>
                            <div className="pattern_type">Butterfly</div>
                            <div className="pattern_type">Bat</div>
                            <div className="pattern_type">Gartley</div>
                            <div className="pattern_type">Crab</div>
                            <div className="pattern_type">Deep Crab</div>
                            <div className="pattern_type">Shark</div>
                            
                        </div>
                        <div className='patterns_settings_right'>
            
                            <div className='filter_box'>
                                    <div className='filter_box_key'>AB  &gt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                            className="input_style"
                                        // value={filters.bc_retracement_greater}
                                    
                                            onChange={(e) => {
                                                // const updatedFilters = {
                                                // ...filters,
                                                // bc_retracement_greater: e.target.value
                                                // };
                                                // set_filters(updatedFilters);  
                                                
                                            }}
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>AB  &lt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                            className="input_style"
                                        // value={filters.bc_retracement_greater}
                                    
                                            onChange={(e) => {
                                                // const updatedFilters = {
                                                // ...filters,
                                                // bc_retracement_greater: e.target.value
                                                // };
                                                // set_filters(updatedFilters);  
                                                
                                            }}
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>BC  &lt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                            className="input_style"
                                        // value={filters.bc_retracement_greater}
                                    
                                            onChange={(e) => {
                                                // const updatedFilters = {
                                                // ...filters,
                                                // bc_retracement_greater: e.target.value
                                                // };
                                                // set_filters(updatedFilters);  
                                                
                                            }}
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>BC  &gt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                        className="input_style"
                                        // value={filters.bc_retracement_greater}
                                    
                                        onChange={(e) => {
                                            // const updatedFilters = {
                                            // ...filters,
                                            // bc_retracement_greater: e.target.value
                                            // };
                                            // set_filters(updatedFilters);  
                                            
                                        }}
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>CD  &gt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                        className="input_style"
                                        // value={filters.bc_retracement_greater}
                                    
                                        onChange={(e) => {
                                            // const updatedFilters = {
                                            // ...filters,
                                            // bc_retracement_greater: e.target.value
                                            // };
                                            // set_filters(updatedFilters);  
                                            
                                        }}
                                        />
                                    
                                    </div>
                                    
                            </div>
                            <div className='filter_box'>
                                    <div className='filter_box_key'>CD  &lt;</div>
                                    <div className='filter_box_val'>
                            
                                        <input
                                        className="input_style"
                                        // value={filters.bc_retracement_greater}
                                    
                                        onChange={(e) => {
                                            // const updatedFilters = {
                                            // ...filters,
                                            // bc_retracement_greater: e.target.value
                                            // };
                                            // set_filters(updatedFilters);  
                                            
                                        }}
                                        />
                                    
                                    </div>
                                    
                            </div>
                        </div>
                        </div>

                    </div>

                </div>
                    
                <div className='table_body_main'>
                    {body}
                </div>

                <div className='table_header_main'>
                
                </div>

            </div>

        </div>
    )
}

export default Section