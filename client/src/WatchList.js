

import React, {useState, useRef, useEffect} from "react"
import dropup from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/arrow_up.png';
import dropdown from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/dropdown.png';
import WLExpand from "./WLExpand";
import * as route from './backend_routes.js'
const Trades = (props) => {

    const {
        all_watchlists,
        set_all_watchlist,
        all_wl_patterns,
        set_chart_data
    } = props

    useEffect(()=>{

        const fetch = async () => {
            const all_watchlist = await route.get_all_watchlist()
            set_all_watchlist(all_watchlist)
        }
        fetch()
        
    },[])

    const [is_expanded, set_is_expanded] = useState(false)
    const inputRef = useRef(null);

    const [wl_data, set_wl_data] = useState([

        {
            watchlist_name: 'Twin Peaks',
            list_: [
                 {
                    symbol: 'AAPL',
                    price: 100.00,
                    change: 50.00,
                    change_pct: 23.05,
                    vol: 235505,
                },
                {
                    symbol: 'TSLA',
                    price: 100.00,
                    change: 50.00,
                    change_pct: 23.05,
                    vol: 235505,
                },
                {
                    symbol: 'AAPL',
                    price: 100.00,
                    change: 50.00,
                    change_pct: 23.05,
                    vol: 235505,
                },

            ]
        },
        {
            watchlist_name: 'Uptrend',
            list_: [
                 {
                    symbol: 'AAPL',
                    price: 100.00,
                    change: 50.00,
                    change_pct: 23.05,
                    vol: 235505,
                },
                {
                    symbol: '',
                    price: 100.00,
                    change: 50.00,
                    change_pct: 23.05,
                    vol: 235505,
                },
                {
                    symbol: 'AAPL',
                    price: 100.00,
                    change: 50.00,
                    change_pct: 23.05,
                    vol: 235505,
                },

            ]
        },
        {
            watchlist_name: 'Downtrend',
            list_: [
                 {
                    symbol: 'AAPL',
                    price: 100.00,
                    change: 50.00,
                    change_pct: 23.05,
                    vol: 235505,
                },
                {
                    symbol: '',
                    price: 100.00,
                    change: 50.00,
                    change_pct: 23.05,
                    vol: 235505,
                },
                {
                    symbol: 'AAPL',
                    price: 100.00,
                    change: 50.00,
                    change_pct: 23.05,
                    vol: 235505,
                },

            ]
        }
        
    ])


    const [is_create, set_create] = useState(false)
    const [input_wl_name, set_input_wl_name] = useState('')


    


    return(
        <div className="wl-wrapper">
            
            <div className="wl-header-wrapper">
                <div className="collapse-butt-wrapper"></div>
                <div className="wl-column">Symbol</div> 
                <div className="wl-column">Price</div>
                <div className="wl-column">Change</div>
                <div className="wl-column">Change &</div>
                <div className="wl-column">Vol</div>
            </div>
            <div className="wl-body-wrapper">

                {all_watchlists?.map((item,key)=>{
                    return(
            
                        <WLExpand 
                            key={key}
                            all_wl_symbols={all_wl_patterns} 
                            name={item} 
                            set_all_watchlist={set_all_watchlist}
                            set_chart_data={set_chart_data}
                        />
                    )
                })}

                {is_create && 
                    
                    <div className="wl-new-row">
                   <input className="w1-new-input" defaultValue="Watchlist Name" autoFocus onChange={(e) => set_input_wl_name(e.target.value)}/>
                    </div>
                }

                <div className="wl-add"
                 onClick={async () => {
                    if (!is_create) {
                        set_create(true);
                    } else {
                        await route.add_watchlist(input_wl_name);
                        const all_watchlist = await route.get_all_watchlist();
                        set_all_watchlist(all_watchlist);
                        set_input_wl_name("");
                        set_create(false);
                    }
                    }}>

                    {is_create ? 'Add Watchlist':'Create Watchlist'}
                    </div>


            </div>

        </div>
    )
}

export default Trades