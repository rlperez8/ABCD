import { useState, useRef, useEffect } from 'react';
import { Grid  } from 'react-window';


const Table = (props) => {

    const {

        selected_pattern_index,
        set_selected_pattern,
        set_selected_pattern_index,
        table,
        abcd_patterns,
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

    const columns = ['trade_result', 'trade_entered_date','trade_entered_price','trade_exited_price','trade_pnl','trade_return_percentage','pattern_ABCD_bar_length']

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

        // if (columnIndex === 0 && content === 'Win') {
        //     cellContent = <div className="first_column_box">{content}</div>;

        // } else if (columnIndex === 0 && content === 'Lost') {
        //     cellContent = <div className="lost_column_box">{content}</div>;
        
        // } else if (columnIndex === 0) {
        //     cellContent = <div className="open_column_box">Open</div>;
        
        // }else if (columnIndex === 5 && Number(content) > 0) {
        //     cellContent = <div className="positive_pnl">${content}</div>;

        // }else if (columnIndex === 5 && Number(content) <= 0) {
        //     cellContent = <div className="negative_pnl">${content}</div>;
        // }

        

        return (
            <div className={selected_row}
                onClick={() => {
         
                    set_selected_pattern(abcd_patterns[rowIndex]);  
                    set_selected_pattern_index(rowIndex)
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
                // columnCount={table.length > 0 ? Object.keys(table[0]).length : 0}
                columnCount={7}
                columnWidth={containerWidth/7}
                rowCount={table?.length}
                rowHeight={40}
                style={{ overflowX: 'none' }} 
            />
        </div>
    )
}

export default Table