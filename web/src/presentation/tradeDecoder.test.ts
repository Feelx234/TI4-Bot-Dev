import { describe, it, expect } from 'vitest';
import { decodeTradeOption } from './tradeDecoder.ts';
import { ChoiceOptionDto } from '../protocol/types.ts';

describe('tradeDecoder', () => {
  it('decodes mutual support for the throne (ss)', () => {
    const opt: ChoiceOptionDto = {
      id: 'ss',
      kind: 'offer',
      label: 'Swap Support for the Throne',
      payload: { net: 1, their_net: 1 },
    };
    const decoded = decodeTradeOption(opt);
    expect(decoded.category).toBe('mutual_support');
    expect(decoded.net).toBe(1);
    expect(decoded.their_net).toBe(1);
  });

  it('decodes commodity swap (cc3)', () => {
    const opt: ChoiceOptionDto = {
      id: 'cc3',
      kind: 'offer',
      label: 'swap 3 commodities each',
      payload: { net: 3, their_net: 3 },
    };
    const decoded = decodeTradeOption(opt);
    expect(decoded.category).toBe('commodity_swap');
    expect(decoded.details.giveCommodities).toBe(3);
    expect(decoded.details.receiveCommodities).toBe(3);
  });

  it('decodes commodity gift (c2:0)', () => {
    const opt: ChoiceOptionDto = {
      id: 'c2:0',
      kind: 'offer',
      label: 'gift 2 commodities',
      payload: { net: -2, their_net: 2 },
    };
    const decoded = decodeTradeOption(opt);
    expect(decoded.category).toBe('commodity_swap');
    expect(decoded.details.giveCommodities).toBe(2);
    expect(decoded.details.receiveCommodities).toBeUndefined();
  });

  it('decodes goods exchange (ct2:3, tc1:2, and 1:2)', () => {
    const ctOpt: ChoiceOptionDto = {
      id: 'ct2:3',
      kind: 'offer',
      label: 'give 2 commodities for 3 trade goods',
      payload: { net: 1, their_net: -1 },
    };
    const decodedCt = decodeTradeOption(ctOpt);
    expect(decodedCt.category).toBe('goods_exchange');
    expect(decodedCt.details.giveCommodities).toBe(2);
    expect(decodedCt.details.receiveTradeGoods).toBe(3);

    const tcOpt: ChoiceOptionDto = {
      id: 'tc1:2',
      kind: 'offer',
      label: 'give 1 trade goods for 2 commodities',
    };
    const decodedTc = decodeTradeOption(tcOpt);
    expect(decodedTc.category).toBe('goods_exchange');
    expect(decodedTc.details.giveTradeGoods).toBe(1);
    expect(decodedTc.details.receiveCommodities).toBe(2);

    const tgOpt: ChoiceOptionDto = {
      id: '1:2',
      kind: 'offer',
      label: 'give 1 for 2',
    };
    const decodedTg = decodeTradeOption(tgOpt);
    expect(decodedTg.category).toBe('goods_exchange');
    expect(decodedTg.details.giveTradeGoods).toBe(1);
    expect(decodedTg.details.receiveTradeGoods).toBe(2);
  });

  it('decodes promissory notes (pnsupport:sol:3)', () => {
    const opt: ChoiceOptionDto = {
      id: 'pnsupport:sol:3',
      kind: 'offer',
      label: 'offer Support for 3 TG',
      payload: { net: 3, their_net: 1 },
    };
    const decoded = decodeTradeOption(opt);
    expect(decoded.category).toBe('promissory');
    expect(decoded.details.promissoryNote).toBe('support:sol');
    expect(decoded.details.price).toBe(3);
  });

  it('decodes action cards and fragments', () => {
    const acOpt: ChoiceOptionDto = {
      id: 'acmorale_boost:2',
      kind: 'offer',
      label: 'offer Morale Boost for 2 TG',
    };
    const decodedAc = decodeTradeOption(acOpt);
    expect(decodedAc.category).toBe('other');
    expect(decodedAc.details.actionCard).toBe('morale_boost');
    expect(decodedAc.details.price).toBe(2);

    const frOpt: ChoiceOptionDto = {
      id: 'frcultural:4',
      kind: 'offer',
      label: 'offer Cultural Fragment for 4 TG',
    };
    const decodedFr = decodeTradeOption(frOpt);
    expect(decodedFr.category).toBe('other');
    expect(decodedFr.details.fragment).toBe('cultural');
    expect(decodedFr.details.price).toBe(4);
  });
});
