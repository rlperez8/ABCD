import { Grid  } from 'react-window';


const Table = (props) => {

    const {

        selected_pattern_index,
        set_selected_pattern,
        set_selected_pattern_index,
        table,
        abcd_patterns,
    } = props



    const CellComponent = ({ columnIndex, rowIndex, style, table }) => {
     
      
        const row = table[rowIndex];  
        if (!row) return <div style={style}></div>;
        const keys = Object.keys(row);
        const columnKey = keys[columnIndex];
        const content = row[columnKey];

        return (
            <div className={rowIndex === selected_pattern_index ? "truncate_active" : 'truncate'}
                onClick={() => {
         
                    set_selected_pattern(abcd_patterns[rowIndex]);  
                    set_selected_pattern_index(rowIndex)
                }}
                style={style}
            >
                {content}
            </div>
        );
    };

    return(
        <div className='table_container'>
        
            <Grid
                className="my-grid"
                cellComponent={CellComponent}
                cellProps={{ table }}
                columnCount={table.length > 0 ? Object.keys(table[0]).length : 0}
                columnWidth={150}
                rowCount={table?.length}
                rowHeight={25}
            />
        </div>
    )
}

export default Table