import React, {useState, useEffect} from "react"


const Trades = (props) => {

    const {

    } = props

    const data = [
        {
            symbol: 'TMHC',
            qty: 11,
            enter: 59.70,
            take_profit: 68.55,
            stop_lost: 55.51,
            rrr: '1:2',
            un_pnl: 0,
            un_pnl_pct: 0
        },
         {
            symbol: 'TMHC',
            qty: 11,
            enter: 59.70,
            take_profit: 68.55,
            stop_lost: 55.51,
            rrr: '1:2',
            un_pnl: 0,
            un_pnl_pct: 0

        }
    ]

    return(
        <div className="wl-wrapper">
            
            <div className="wl-header-wrapper">
                <div className="wl-column">Symbol</div> 
                <div className="wl-column">QTY</div> 
                <div className="wl-column">Enter</div>
                <div className="wl-column">Take Profit</div>
                <div className="wl-column">Stop Lost</div>
                <div className="wl-column">RRR</div>
                <div className="wl-column">UR P&L</div>
                <div className="wl-column">UR P&L %</div>
            </div>
            <div className="wl-body-wrapper">

                {data.map(item=>{
                    return(
                   <div className="wl-body-row">
                        <div className="wl-column">{item.symbol}</div> 
                        <div className="wl-column">{item.qty}</div> 
                        <div className="wl-column">{item.enter}</div> 
                        <div className="wl-column">{item.take_profit}</div> 
                        <div className="wl-column">{item.stop_lost}</div> 
                        <div className="wl-column">{item.rrr}</div> 
                        <div className="wl-column">{item.un_pnl}</div> 
                        <div className="wl-column">{item.un_pnl_pct}</div> 
                    </div>
                            
                    
                    )
                })}


            </div>

        </div>
    )
}

export default Trades