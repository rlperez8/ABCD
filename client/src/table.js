import { useState, useRef, useEffect } from 'react';
import { Grid  } from 'react-window';
import * as route from './backend_routes.js';
const Table = (props) => {

    const {

        selected_pattern_index,
        set_selected_pattern,
        set_selected_pattern_index,
        table,
        abcd_patterns,
        set_candles,
        set_chart_data,
 
    } = props

    const containerRef = useRef(null);
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



    const [hovered_row_index, set_hovered_index] = useState(0)

    // const columns = ['trade_result', 'trade_entered_date','trade_entered_price','trade_exited_price','trade_pnl','trade_return_percentage','pattern_ABCD_bar_length']
    const columns = ['trade_result', 'symbol','pattern_AB_bar_length','pattern_BC_bar_length','pattern_CD_bar_length','trade_entered_date','trade_duration_bars']

    const CellComponent = ({ columnIndex, rowIndex, style, table }) => {
     
       
        const row = table[rowIndex];  
        if (!row) return <div style={style}></div>;
        const keys = Object.keys(row);
        const columnKey = columns[columnIndex];
        const content = row[columnKey];

        // Selected Row
        
        const selected_row = rowIndex === hovered_row_index || rowIndex === selected_pattern_index ? 'ticker_row_selected' 
                : rowIndex % 2 === 0
                ? 'ticker_row_even'
                : 'ticker_row_odd';

             
        
                        


        let cellContent = content;

        if (columnIndex === 0 && content === 'Win') {
            cellContent = <div className="first_column_box">{content}</div>;

        } else if (columnIndex === 0 && content === 'Lost') {
            cellContent = <div className="lost_column_box">{content}</div>;
        
        } else if (columnIndex === 0) {
            cellContent = <div className="open_column_box">Open</div>;
        
        }else if (columnIndex === 5 && Number(content) > 0) {
            cellContent = <div className="positive_pnl">${content}</div>;

        }else if (columnIndex === 5 && Number(content) <= 0) {
            cellContent = <div className="negative_pnl">${content}</div>;
        }

        

        return (
            <div className={selected_row}
                onClick={() => {

                    const fetchData = async () => {
                        try {
                      
                            // set_ticker_symbol(table[rowIndex].symbol);
              
                      
                            const [candles] = await Promise.all([
                                route.get_candles(table[rowIndex].symbol),
                
                            ]);

                            candles.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));

                            return { candles };
                        } catch (error) {
                            console.error('Error fetching data:', error);
                            return { candles: [], abcd_patterns: [] };
                        } 
                    };
                      fetchData().then(({ candles }) => {

                            set_candles(candles);
                            set_selected_pattern(abcd_patterns[rowIndex]);  
                            set_selected_pattern_index(rowIndex);
                            set_chart_data({
                            abcd_pattern: abcd_patterns[rowIndex],
                            candles: candles,
                            });
                     
              
                        });

                   
        
                }}
                style={style}
                onMouseEnter={() => {
                    set_hovered_index(rowIndex);
                    // console.log("Hovered row:", rowIndex);
                    }}
            >
                {cellContent}
            </div>
        );
    };

    return(
        <div className='table_container' ref={containerRef}>
        
            <Grid
                    className="my-grid"
                    cellComponent={CellComponent}
                    cellProps={{ table }}
                    columnCount={7}
                    columnWidth={(containerWidth || 700) / 7} // default width in px if not set
                    rowCount={table?.length || 0}
                    rowHeight={40}
                    width={containerWidth || 700} // default width if containerWidth is 0
                    height={Math.min((table?.length || 0) * 40, 600)} // default 0 rows
                    style={{ overflowX: 'hidden' }}
                    />
        </div>
    )
}

export default Table