import React, {useState} from "react";


const FilterDropDown = (props) => {

    const {
        name,
        activateMarket,
        options,
        set_selected_filter,
        selected_filter,
        open
    } = props

    const [value, setValue] = useState("");
   

return(

    <div className="setting-container">
        {/* <div className="setting-key">{name}:</div> */}

        <div className="setting-value-outer">
            <div className="setting-value-inner" 
                onClick={() => set_selected_filter(selected_filter === name ? '' : name)}>

                <div className="value-name"> {value} </div>
                <div className="value-arrow">
                    <img className='value-arrow-img'src="/images/dropdown.png"></img>
                </div>
    


            </div>

            {open && (
            <div className="setting-dropdown">
                {options.map(opt => (
                <div
                    key={opt}
                    className="setting-dropdown-item"
                    onClick={() => {
                    setValue(opt);
                    activateMarket(opt);
                    set_selected_filter("")
                    }}
                >
                    {opt}
                </div>
                ))}
            </div>
            )}
        </div>
    </div>

)
    
}

export default FilterDropDown