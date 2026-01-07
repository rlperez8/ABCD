import { Listbox } from "@headlessui/react";
import { useState } from "react";

const markets = ["All", "NYSE", "NASDAQ"];

export default function MarketDropdown() {
  const [selected, setSelected] = useState(markets[0]);

  return (
    <div className="setting-key">
      <Listbox value={selected} onChange={setSelected}>
        <Listbox.Button className="w-full px-3 py-2 bg-[#2C3136] text-white rounded border border-[#3A3F45] text-left">
          {selected}
        </Listbox.Button>

        <Listbox.Options className="absolute mt-1 w-full bg-[#2C3136] border border-[#3A3F45] rounded shadow-lg z-50">
          {markets.map((market) => (
            <Listbox.Option
              key={market}
              value={market}
              className={({ active }) =>
                `px-3 py-2 cursor-pointer ${
                  active ? "bg-[#32373D]" : ""
                }`
              }
            >
              {market}
            </Listbox.Option>
          ))}
        </Listbox.Options>
      </Listbox>
    </div>
  );
}
