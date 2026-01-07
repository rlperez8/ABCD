// import React,{useEffect, useState} from "react";
// import settings from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/SETTINGS.png';
// import dropdown from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/dropdown.png';
// import dropup from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/arrow_up.png';
// const Filter = (props) => {

//     const {
//         filters,
//         set_filters,
//         fetch_filtered_peformances,
//         set_ticker_peformance,
//         is_loading,
//         set_loading,
//         set_selected_pattern_index,
//         handle_updated_recent_patterns
//     } = props
    

//     const [selected_type, set_type] = useState('')
//     const [scale, set_scale] = useState('')
//     useEffect(() => {
//         if (!filters) return;

//         const fetchData = async () => {
//             set_loading(true)
//             const filtered_data = await fetch_filtered_peformances(filters);
//             set_ticker_peformance(filtered_data)
  
  
//             set_loading(false)

            
//         };
//         // fetchData();
//     }, []);

//     const abcd_types_ = [
//          {
//             type: 'Standard',
//             ratios: {
//             bc_retracement_greater: 38.2,
//             bc_retracement_less: 61.8,
//             cd_retracement_greater: 100,
//             cd_retracement_less: 127,
//             }
//         },
//         {
//             type: 'Extended',
//             ratios: {
//             bc_retracement_greater: 38.2,
//             bc_retracement_less: 61.8,
//             cd_retracement_greater: 127,
//             cd_retracement_less: 161.8,
//             }
//         },
//         {
//             type: 'Contracted',
//             ratios: {
//             bc_retracement_greater: 61.8,
//             bc_retracement_less: 78.6,
//             cd_retracement_greater: 78.6,
//             cd_retracement_less: 100,
//             }
//         },

//     ]
//     const abcd_scale_types = [
//   {
//     scale: 'Micro',
//     legs: {
//       ab_leg_greater: 0,
//       ab_leg_less: 9,
//       bc_leg_greater: 0,
//       bc_leg_less: 9,
//       cd_leg_greater: 0,
//       cd_leg_less: 9
//     }
//   },
//   {
//     scale: 'Tiny',
//     legs: {
//       ab_leg_greater: 10,
//       ab_leg_less: 20,
//       bc_leg_greater: 10,
//       bc_leg_less: 20,
//       cd_leg_greater: 10,
//       cd_leg_less: 20
//     }
//   },
//   {
//     scale: 'Small',
//     legs: {
//       ab_leg_greater: 21,
//       ab_leg_less: 50,
//       bc_leg_greater: 21,
//       bc_leg_less: 50,
//       cd_leg_greater: 21,
//       cd_leg_less: 50
//     }
//   },
//   {
//     scale: 'Medium',
//     legs: {
//       ab_leg_greater: 51,
//       ab_leg_less: 200,
//       bc_leg_greater: 51,
//       bc_leg_less: 200,
//       cd_leg_greater: 51,
//       cd_leg_less: 200
//     }
//   },
//   {
//     scale: 'Large',
//     legs: {
//       ab_leg_greater: 201,
//       ab_leg_less: 1000,
//       bc_leg_greater: 201,
//       bc_leg_less: 1000,
//       cd_leg_greater: 201,
//       cd_leg_less: 1000
//     }
//   }
//     ];

//     const abcd_types = [
//         'Standard',
//         'Extended',
//         // 'Contracted',
//         // 'Bat',
//         // 'Gartley',
//         // 'Butterfly',
//         // 'Crab',
//         // 'Deep Crab',
//         // 'Shark',
//         // 'Cypher',
//         // '5-0',
//         // 'Three Drives'
//     ];

//     const handle_ = (selectedType) => {

    

//         const pattern = abcd_types_.find(p => p.type === selectedType);

//         if (!pattern) return; 

//         const updatedFilters = {
//             ...filters,
//             ...pattern.ratios
//         };
//         set_type(selectedType);
  
//         return updatedFilters
       
//     };


//     function updateLegFilters(selectedScale) {
//         const scale = abcd_scale_types.find(s => s.scale === selectedScale);
//         if (!scale) return;

//         const updatedFilters = {
//             ...filters,
//             ...scale.legs
//         };

//         set_scale(selectedScale);
//         set_filters(updatedFilters);
//     }

//     return(
//         <div className='table_body_main' >
//             <div className="filts">

//                 <div className="filter_left">

//                     <div className='abcd_types_filter_container'>

//                         {abcd_types.map((item,index)=>{
//                             return(
//                                 <div 
//                                     key={index}
//                                     className={selected_type === item? 'abcd_types_filter_selected' : 'abcd_types_filter'} 
//                                     onClick={()=> {handle_(item)}}>

//                                     {item}
//                                 </div>

//                             )
//                         })}

                     

//                     </div>

                 

//                 </div>

//                 {/* <div className="filter_left">

//                     <div className='abcd_types_filter_container'>

//                         {Object.keys(abcd_scale_types).map((item,index)=>{
//                             return(
//                                 <div 
//                                 key={index}
//                                 className={scale === abcd_scale_types[item].scale? 'abcd_types_filter_selected' : 'abcd_types_filter'} 
//                                 onClick={()=> {updateLegFilters(abcd_scale_types[item].scale)}}>
//                                     {abcd_scale_types[item].scale}
//                                 </div>

//                             )
//                         })}

//                     </div>

//                 </div>
//              */}
//                 <div className="filter_right">

//                     <div className="filter-box-header">Price Retracement</div>
                

//                     <div className='filter_box'>
//                         <div className='filter_box_key'>BC  &gt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.bc_retracement_greater}
                        
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 bc_retracement_greater: e.target.value
//                                 };
//                                 set_filters(updatedFilters);  
                                
//                             }}
//                             />
                        
//                         </div>
                        
//                     </div>

//                     <div className='filter_box'>
//                         <div className='filter_box_key'>BC  &lt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.bc_retracement_less}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 bc_retracement_less: e.target.value
//                                 };

//                                 set_filters(updatedFilters);  
                    
//                             }}
//                             />
                        
//                         </div>
                        
//                     </div>
            
//                     <div className='filter_box'>
//                         <div className='filter_box_key'>CD &gt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.cd_retracement_greater}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 cd_retracement_greater: e.target.value
//                                 };

//                                 set_filters(updatedFilters);  
                    
//                             }}
//                             />
                        
//                         </div>
//                     </div>
                    
//                     <div className='filter_box'>
//                         <div className='filter_box_key'>CD &lt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.cd_retracement_less}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 cd_retracement_less: e.target.value
//                                 };
//                                 set_filters(updatedFilters);  

                
                
//                             }}
//                             />
                        
//                         </div>
//                     </div>
                    

//                 </div>

//                 <div className="filter_right">

//                     <div className="filter-box-header">Leg Length</div>
                

//                     <div className='filter_box'>
//                         <div className='filter_box_key'>BC &gt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.bc_retracement_greater}
                        
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 bc_retracement_greater: e.target.value
//                                 };
//                                 set_filters(updatedFilters);  
                                
//                             }}
//                             />
                        
//                         </div>
                        
//                     </div>

//                     <div className='filter_box'>
//                         <div className='filter_box_key'>BC  &lt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.bc_retracement_less}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 bc_retracement_less: e.target.value
//                                 };

//                                 set_filters(updatedFilters);  
                    
//                             }}
//                             />
                        
//                         </div>
                        
//                     </div>
            
//                     <div className='filter_box'>
//                         <div className='filter_box_key'>CD &gt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.cd_retracement_greater}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 cd_retracement_greater: e.target.value
//                                 };

//                                 set_filters(updatedFilters);  
                    
//                             }}
//                             />
                        
//                         </div>
//                     </div>
                    
//                     <div className='filter_box'>
//                         <div className='filter_box_key'>CD &lt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.cd_retracement_less}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 cd_retracement_less: e.target.value
//                                 };
//                                 set_filters(updatedFilters);  

                
                
//                             }}
//                             />
                        
//                         </div>
//                     </div>
                    

//                 </div>

//                 <div className="filter_right">

//                     <div className="filter-box-header">Leg Length</div>
                

//                     <div className='filter_box'>
//                         <div className='filter_box_key'>BC &gt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.bc_retracement_greater}
                        
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 bc_retracement_greater: e.target.value
//                                 };
//                                 set_filters(updatedFilters);  
                                
//                             }}
//                             />
                        
//                         </div>
                        
//                     </div>

//                     <div className='filter_box'>
//                         <div className='filter_box_key'>BC  &lt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.bc_retracement_less}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 bc_retracement_less: e.target.value
//                                 };

//                                 set_filters(updatedFilters);  
                    
//                             }}
//                             />
                        
//                         </div>
                        
//                     </div>
            
//                     <div className='filter_box'>
//                         <div className='filter_box_key'>CD &gt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.cd_retracement_greater}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 cd_retracement_greater: e.target.value
//                                 };

//                                 set_filters(updatedFilters);  
                    
//                             }}
//                             />
                        
//                         </div>
//                     </div>
                    
//                     <div className='filter_box'>
//                         <div className='filter_box_key'>CD &lt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.cd_retracement_less}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 cd_retracement_less: e.target.value
//                                 };
//                                 set_filters(updatedFilters);  

                
                
//                             }}
//                             />
                        
//                         </div>
//                     </div>
                    

//                 </div>

//                 <div className="filter_right">

//                     <div className="filter-box-header">Leg Length</div>
                

//                     <div className='filter_box'>
//                         <div className='filter_box_key'>BC &gt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.bc_retracement_greater}
                        
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 bc_retracement_greater: e.target.value
//                                 };
//                                 set_filters(updatedFilters);  
                                
//                             }}
//                             />
                        
//                         </div>
                        
//                     </div>

//                     <div className='filter_box'>
//                         <div className='filter_box_key'>BC  &lt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.bc_retracement_less}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 bc_retracement_less: e.target.value
//                                 };

//                                 set_filters(updatedFilters);  
                    
//                             }}
//                             />
                        
//                         </div>
                        
//                     </div>
            
//                     <div className='filter_box'>
//                         <div className='filter_box_key'>CD &gt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.cd_retracement_greater}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 cd_retracement_greater: e.target.value
//                                 };

//                                 set_filters(updatedFilters);  
                    
//                             }}
//                             />
                        
//                         </div>
//                     </div>
                    
//                     <div className='filter_box'>
//                         <div className='filter_box_key'>CD &lt;</div>
//                         <div className='filter_box_val'>
                
//                             <input
//                             className="input_style"
//                             value={filters.cd_retracement_less}
//                             onChange={(e) => {
//                                 const updatedFilters = {
//                                 ...filters,
//                                 cd_retracement_less: e.target.value
//                                 };
//                                 set_filters(updatedFilters);  

                
                
//                             }}
//                             />
                        
//                         </div>
//                     </div>
                    

//                 </div>
                
//                 <div className="save-filter-wrapper" onClick={()=>{set_filters(handle_(selected_type))}}>Build</div>
                

//             </div>
//         </div>
//     )
// }


// export default Filter