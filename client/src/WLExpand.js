


import React, {useState, useEffect} from "react"
import delete_ from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/delete.png';
import dropdown from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/dropdown.png';
import right_arrow from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/right_arrow.png';
import * as route from './backend_routes.js'

const WLExpand = (props) => {

    const {
        all_wl_symbols,
        name,
        set_all_watchlist,
        set_chart_data
    } = props


    const [is_expanded, set_is_expanded] = useState(false)


    return(
        <div className={is_expanded ? 'wl-expanded' : 'wl-collapsed'}
            >

            <div className="collapsed-header" onClick={()=>{set_is_expanded(!is_expanded)}}>
                <div className="collapse-butt-wrapper">
                    <img className='icon_img' src={is_expanded ? dropdown : right_arrow}/>
                </div>
                <div className="collapse-watchlist-name">
                    {name.wl_name}
                </div>
                  <div className="collapse-watchlist-delete" 
                    onClick={async () => {
                        await route.delete_watchlist(name.wl_name)
                        const all_watchlist = await route.get_all_watchlist()
                        set_all_watchlist(all_watchlist)
                    }}>
                   <img className='delete_img' src={delete_}/>
                </div>
        
        

            </div>

            <div className="watchlist-body">
                {all_wl_symbols?.map((item, key) => {
                  
                    if (item.wl_name === name.symbol) {
                    return (
                        <div className="wl-header-wrapper" onClick={()=>{

                    const fetch = async () => {
                        console.log(item)
                        // const candles = await route.get_candles(item.symbol)

                        set_chart_data(prev => ({
                        ...prev,
                        // candles: candles,
                        abcd_pattern: item
                        }));
                            
                    }

                    fetch()


                    
                                    
                }} key={key}>
                        <div className="wl-column">{item.symbol}</div>
                        {/* <div className="wl-column">{item.price}</div>
                        <div className="wl-column">{item.change}</div>
                        <div className="wl-column">{item.change_pct}</div>
                        <div className="wl-column">{item.volume}</div> */}
                        </div>
                    );
                    }
                    // Otherwise return null
                    return null;
                })}
                </div>


        </div>



    )
}

export default WLExpand