import { useEffect, useState } from 'react';
import Table from './table';
import settings from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/SETTINGS.png';
import dropdown from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/dropdown.png';
import dropup from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/arrow_up.png';
import edit from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/edit.png';
import calendar from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/calendar.png';
const PatternTable = (props) => {

    const {

        table,
        selected_pattern_index,
        set_selected_pattern,
        set_selected_pattern_index,
        abcd_patterns,
        is_loading_patterns,
        sort_patterns,
        set_sorted_abcd_patterns,
        sorted_abcd_patterns
    } = props
    
    const [dropbox_active, set_dropbox_active] = useState(false)
    const [selected_pivot, set_selected_pivot] = useState('A')
    const [pivot_dates, set_pivot_dates] = useState([])
    const [current_date, set_date] = useState('10-10-2025')
    const [date_obj, set_date_obj] = useState('pattern_A_pivot_date')
    const [selected_A_date, set_selected_A_date] = useState('')
    const [selected_B_date, set_selected_B_date] = useState('')
    const [selected_C_date, set_selected_C_date] = useState('')
    const [selected_D_date, set_selected_D_date] = useState('')
    const [calendar_active, se_calendar_active] = useState(false)

    const date_dropdown = [
        {
            date: 'A',
            key: 'pattern_A_pivot_date'
        },
        {
            date: 'B',
            key: 'pattern_B_pivot_date'
        },
        
        {
            date: 'C',
            key: 'pattern_C_pivot_date'
        },
        
        {
            date: 'D',
            key: 'trade_entered_date'
        },
        {
            date: 'EX',
            key: 'trade_exited_date'
        },
        

    ]
    const sort_patterns_by_dates = (date) => {
   
        const filtered = sorted_abcd_patterns.filter(pattern => {

            // Convert both to comparable date strings
            const pivot = new Date(pattern[date_obj]);
            const selected = new Date(date);

            // Compare by date only (ignore time)
            return pivot.toDateString() === selected.toDateString();
        });

      
        set_sorted_abcd_patterns(filtered)

    };

    const get_css_for_selected_date = (date) => {
        if (selected_pivot === 'A' && selected_A_date === date) {
            return 'dropdown-option-active';
        } else if (selected_pivot === 'B' && selected_B_date === date) {
            return 'dropdown-option-active';
        } else if (selected_pivot === 'C' && selected_C_date === date) {
            return 'dropdown-option-active';
        } else if (selected_pivot === 'D' && selected_D_date === date) {
            return 'dropdown-option-active';
        } else {
            return 'dropdown-option';
        }
        };

    useEffect(() => {
        if (!sorted_abcd_patterns || sorted_abcd_patterns.length === 0) return;

        const dates = sorted_abcd_patterns.map(item => item[date_obj]);
        const uniqueDates = [...new Set(dates)];

        set_pivot_dates(uniqueDates);

    }, [table,date_obj,sorted_abcd_patterns]);


    const [is_collapse, set_collapse] = useState(false)
    return(
        <div className={is_collapse ? 'patterns_table_main_expanded' : 'patterns_table_main' }>

            <div className='table_header_main'>
                <div className='table_header_text'>Patterns</div>
                 <div className='settings_icon'># {sorted_abcd_patterns?.length}</div>
                <div className='settings_icon' onClick={()=>{set_collapse(!is_collapse)}}>
                    <img className='icon_img' src={is_collapse ? dropdown : dropup}/>
                </div>
            </div>

            <div className='patterns_settings'>

                <div className='filtter_selection_container'>

                    <div className={calendar_active ? 'filter-date-active': 'filter-date'}>

                        <div className='date-icon' onClick={()=>{set_dropbox_active(!dropbox_active);se_calendar_active(!calendar_active)}}>
                                <img className='icon_img' src={calendar}/>
                        </div>
                        
                        {dropbox_active && (
                            <div className='dropdown-dates'>

                                <div className='dropdown-left'>
                                    {Object.keys(date_dropdown).map((val,index)=>{
                                        return(
                                            <div 
                                                className={selected_pivot === date_dropdown[index].date ? 'dropdown-option-active' : 'dropdown-option'}
                                                onClick={()=>{
                                                    set_selected_pivot(date_dropdown[index].date);
                                                    set_date_obj(date_dropdown[index].key);
                                                }}>
                                                    {date_dropdown[index].date}
                                            </div>
                                        )
                                    })}


                                </div>
                                <div className='dropdown-right'>

                                

                                    <div className='dropdown-dates-list'>

                                    {pivot_dates.map(date => (
                                        <div 
                                            key={date}  
                                            className={get_css_for_selected_date(date)}
                                            onClick={() => {
                                            sort_patterns_by_dates(date); 
                                    

                                            if (selected_pivot === 'A') set_selected_A_date(date);
                                            else if (selected_pivot === 'B') set_selected_B_date(date);
                                            else if (selected_pivot === 'C') set_selected_C_date(date);
                                            else if (selected_pivot === 'D') set_selected_D_date(date);
                                            }}
                                        >
                                            {date}
                                        </div>
                                    ))}
                                </div>

                                        <div className='dropdown-clear' 
                                        onClick={()=>{set_sorted_abcd_patterns(table)}}>
                                        
                                        Clear
                                    
                                    </div>
                                </div>
                                
                            

                                
                            </div>
                        )}         

                    </div>
                </div>

            </div>

                  <div className='peformance_table_header'>
                    <div className='ticker_column' >Result</div>
                    <div className='ticker_column'>Enter Date.</div>
                    <div className='ticker_column'>Enter Price</div>
                    <div className='ticker_column'>Exit Price</div>
                    <div className='ticker_column'>PNL</div>
                    <div className='ticker_column'>Return %</div>
                    <div className='ticker_column'>Length</div>
        
                </div>

            <div className='table_body_main'>

                {is_loading_patterns && <div className='overlay'>
                    <div className='loading_container'>Loading...</div>
    

                    </div>}

          

                <div className='patterns_table_inner'>
                
                        <Table 
                            selected_pattern_index={selected_pattern_index}
                            set_selected_pattern={set_selected_pattern}
                            set_selected_pattern_index={set_selected_pattern_index}
                            table={sorted_abcd_patterns}
                            abcd_patterns={sorted_abcd_patterns}
                        />
                
                </div> 

            </div>

        </div>)
                            
        
}


export default PatternTable