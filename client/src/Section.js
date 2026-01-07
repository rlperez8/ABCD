import React, {useState} from "react"
import dropup from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/arrow_up.png';
import dropdown from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/dropdown.png';
import InfiniteTable from "./InfiniteTable";
import PatternFiltering from "./filter/pattern_filtering";

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
    const activateSNR = value => {

        setActiveFilters(prev => ({
            ...prev,
            snr: { 
                active: true, 
                filter: value 
            }
        }));
    };
    

    return(
        <div className={is_collapse ? 'patterns_table_main' : get_css() }>
            
            <div className="margin-">
        
                {/* <div className='table_header_main'>
                    <div className='table_header_text'>{title}</div>
                    
                    <div className='table_header_count'>{length}</div>
                    <div className='settings_icon' onClick={()=>{set_collapse(!is_collapse)}}>
                    <img className='icon_img' src={is_collapse ? dropdown : dropup}/>
                    </div>
                </div> */}

                {/* <PatternFiltering 
                    activeFilters={activeFilters}  
                    activateResult={activateResult}  
                    activateMarket={activateMarket}
                    activateSNR={activateSNR}
                    harmonic_patterns={harmonic_patterns}
                    activateRetracement={activateRetracement}
                /> */}

                    
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