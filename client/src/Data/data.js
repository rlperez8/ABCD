
import React from "react"

const Data = (props) => {

    const {
        patterns,

    } = props

    const TOL = {
        ab_xa: 2,    
        ad_xa: 2
    };

    const withinTolerance = (value, target, tolerancePercent) => {
        return Math.abs(value - target) <= tolerancePercent;
    };

    let gartley = patterns.filter(p =>
        withinTolerance(p.trade_ab_price_retracement, 61.8, TOL.ab_xa) &&
        p.trade_bc_price_retracement >= 38.2 &&
        p.trade_bc_price_retracement <= 88.6 &&
        p.trade_cd_bc_price_retracement >= 127.2 &&
        p.trade_cd_bc_price_retracement <= 161.8 &&
        withinTolerance(p.trade_cd_xa_price_retracement, 78.6, TOL.ad_xa)
    );


    let rust_gartley = patterns.filter(p => p.harmonic_type === "Gartley")

   
    
    return(
        <></>
    )
}

export default Data