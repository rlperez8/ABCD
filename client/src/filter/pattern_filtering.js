import React from "react"


const PatternFiltering = (props) => {

    const {
        activeFilters,
        activateResult,
        activateMarket,
        activateSNR,
        harmonic_patterns,
        activateRetracement
    } = props



    return(
        <div className='patterns_settings'>

            <div className="market-wrapper">
                <div className="retracement-header">Market</div>
                
                <div className={activeFilters.market.filter === 'Bullish' ? "market-selected" : 'market-'} onClick={() => activateMarket("Bullish")}>Bullish</div>
                <div className={activeFilters.market.filter === 'Bearish' ? "market-selected" : 'market-'} onClick={() => activateMarket("Bearish")}>Bearish</div>

                 <div className="retracement-header">Status</div>
                <div className={activeFilters.result.filter  === 'true' ? "market-selected" : 'market-'}  onClick={() =>{activateResult("true")}}>Won</div>
                <div className={activeFilters.result.filter  === 'false' ? "market-selected" : 'market-'}  onClick={()=>{activateResult("false")}}>Lost</div>
                <div className={activeFilters.result.filter  === 'Open' ? "market-selected" : 'market-'}  onClick={()=>{activateResult("Open")}}>Open</div>
                

            </div>



            <div className="market-wrapper">
                <div className="retracement-header">Support & Resistance</div>
                <div className={activeFilters.snr.filter === 3 ? "market-selected" : 'market-'}  onClick={() =>{activateSNR(3)}}>3 Months</div>
                <div className={activeFilters.snr.filter === 6 ? "market-selected" : 'market-'}  onClick={()=>{activateSNR(6)}}>6 Months</div>
                <div className={activeFilters.snr.filter === 12 ? "market-selected" : 'market-'}  onClick={()=>{activateSNR(12)}}>12 Months</div>
                {/* <div className={false ? "market-selected" : 'market-'}  onClick={()=>{activateSNR("Open")}}>None</div> */}

            </div>


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
             
        </div>

    )
}


export default PatternFiltering