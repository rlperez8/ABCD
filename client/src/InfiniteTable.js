import React, {useState, useEffect, useRef} from "react"
import { Grid  } from 'react-window';
import * as route from './backend_routes.js';
import * as tools from './MainTools.js'

const InfiniteTable = (props) => {

    const {
        recent_patterns,
        set_loading_patterns,
        set_chart_data,
        selected_row_index,
        set_selected_row_index,
        update_selected_pattern
    } = props
    

    const sorted = recent_patterns
    const containerRef = useRef(null);
    const [hovered_row_index, set_hovered_index] = useState(0)
   
    const columns = [
        'trade_result', 
        'symbol',
        'd_date', 
        'trade_enter_price',
        // 'trade_current_price',
        // 'price_change',
        // 'price_change_pct', 
        // 'trade_pnl',
        // 'trade_pnl_pct',
        // 'trade_bc_price_retracement',
        // 'trade_cd_price_retracement',
        // 'pattern_A_pivot_date'
    
    ]
    const [containerWidth, setContainerWidth] = useState(0);

    useEffect(() => {
        if (containerRef.current) {
        setContainerWidth(containerRef.current.offsetWidth);
        }

        // Optional: update on window resize
        const handleResize = () => {
        if (containerRef.current) {
            setContainerWidth(containerRef.current.offsetWidth);
        }
        };
        window.addEventListener('resize', handleResize);
        return () => window.removeEventListener('resize', handleResize);
    }, []);

    const CellComponent = ({ columnIndex, rowIndex, style, sorted }) => {

            const row = sorted[rowIndex];  
            if (!row) return <div style={style}></div>;
            const keys = Object.keys(row);
            const columnKey = columns[columnIndex];
            const content = row[columnKey];

            const selected_row = rowIndex === hovered_row_index || rowIndex === selected_row_index? 'ticker_row_selected' 
                : rowIndex % 2 === 0
                ? 'ticker_row_even'
                : 'ticker_row_odd';

                
            let cellContent = content;

            if (columnKey === 'pct_diff_from_snr' && content !== null && content !== undefined) {
                cellContent = Number(content).toFixed(2);
            }


            if (columnIndex === 0 && content === '1') {
                cellContent = <div className="first_column_box">Won</div>;

            } else if (columnIndex === 0 && content === '0') {
                cellContent = <div className="lost_column_box">Lost</div>;
            
            } else if (columnIndex === 0) {
                cellContent = <div className="open_column_box">Open</div>;
            
            }
            else if ((columnIndex === 7 || columnIndex === 8) && Number(cellContent) > 0) {
                cellContent = <div className="number">{cellContent.toFixed(2)}</div>;

            }
            else if ((columnIndex === 7 || columnIndex === 8)  && Number(content) <= 0) {
                cellContent = <div className="negative_pnl">${cellContent}</div>;
            }
          else if (columnIndex === 7 || columnIndex === 8) {
            const date = new Date(cellContent);

            // Add 1 day (in milliseconds)
            date.setDate(date.getDate() + 1);

            const formattedDate = date
                .toLocaleDateString("en-US", {
                    year: "2-digit",
                    month: "2-digit",
                    day: "2-digit",
                })
                .replace(/\//g, "-"); // Replace slashes with dashes

            cellContent = <div className="negative_pnl">{formattedDate}</div>;
        }


            return (
                <div className={selected_row}
                    onClick={async () => {
                        try {
                            set_loading_patterns(true);
                            set_selected_row_index(rowIndex);

                            const selected = sorted?.[rowIndex];
                            if (!selected) throw new Error("Selected row not found");

                            await update_selected_pattern(selected, set_chart_data);

                        } catch (error) {
                            console.error("Error fetching data:", error);
                            set_chart_data({ selected: null, candles: [] });
                        } finally {
                            set_loading_patterns(false);
                        }
                        }}

                    style={style}
                    onMouseEnter={() => {
                        set_hovered_index(rowIndex);
                        }}
                >
                 <div>
                    {typeof cellContent === 'number' ? cellContent.toFixed(2) : cellContent}
                    </div>

                </div>
            );
    };
    
    return(
        <div className='table_container' ref={containerRef}>

            

            <div className='peformance_table_header'>
                <div className='ticker_column'>Result</div>
                <div className='ticker_column'>Symbol</div>
                <div className='ticker_column'>Enter Date</div>
                <div className='ticker_column'>Enter Price</div>
                {/* <div className='ticker_column'>Price</div>
                <div className='ticker_column'>Change</div>
                <div className='ticker_column'>Change %</div> */}
                {/* <div className='ticker_column'>UNR-PNL</div>
                <div className='ticker_column'>UNR-PNL %</div>
                <div className='ticker_column'>BC</div>
                <div className='ticker_column'>CD</div>
               */}
              
              
       
            </div>

            {recent_patterns?.length > 0 ? (
        
            <Grid
                className="my-grid"
                cellComponent={CellComponent}
                cellProps={{ sorted }}
                columnCount={4}
                columnWidth={(containerWidth || 700) / 4} 
                rowCount={sorted?.length || 0}
                rowHeight={40}
                width={containerWidth || 700} 
                height={Math.min((recent_patterns?.length || 0) * 40, 600)} 
                style={{ overflowX: 'hidden' }}
            />) : ""}

              <div className='peformance_table_header'>
        
              
       
            </div>

        </div>
    )
}


export default InfiniteTable