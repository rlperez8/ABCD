import Table from './table';

const PatternTable = (props) => {

    const {

        table,
        selected_pattern_index,
        set_selected_pattern,
        set_selected_pattern_index,
        abcd_patterns,
        is_loading_patterns,
   


    } = props

  

    return(
    
        <div className='patterns_table_main'>

              {is_loading_patterns && <div className='overlay'>
                      <div className='loading_container'>Loading...</div>
    

                    </div>}

            <div className='peformance_table_header'>
                <div className='ticker_column'>Result</div>
                <div className='ticker_column'>Enter Date.</div>
                {/* <div className='ticker_column'>Exit Date</div> */}
                <div className='ticker_column'>Enter Price</div>
                <div className='ticker_column'>Exit Price</div>
                <div className='ticker_column'>PNL</div>
                <div className='ticker_column'>Return %</div>
                <div className='ticker_column'>Length</div>
                {/* <div className='ticker_column'></div> */}
            </div>
            
            <div className='patterns_table_inner'>
            
                    <Table 
                        selected_pattern_index={selected_pattern_index}
                        set_selected_pattern={set_selected_pattern}
                        set_selected_pattern_index={set_selected_pattern_index}
                        table={table}
                        abcd_patterns={abcd_patterns}
                    />
             
            </div>

        </div> )
}


export default PatternTable