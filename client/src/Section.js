import React, {useState} from "react"
import dropup from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/arrow_up.png';
import dropdown from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/dropdown.png';
import InfiniteTable from "./InfiniteTable";
const Section = (props) => {

     const {
        title,
        body,
        length
    } = props

    const [is_collapse, set_collapse] = useState(false)

    return(
        <div className={is_collapse ? 'patterns_table_main_expanded' : 'patterns_table_main' }>
    
            <div className='table_header_main'>
                <div className='table_header_text'>{title}</div>
                
                <div className='table_header_count'>{length}</div>
                <div className='settings_icon' onClick={()=>{set_collapse(!is_collapse)}}>
                <img className='icon_img' src={is_collapse ? dropdown : dropup}/>
                </div>
            </div>

            <div className='patterns_settings'>
                
            </div>
                
            <div className='table_body_main'>
                {body}
            </div>
                        
        </div>
    )
}

export default Section