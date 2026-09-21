import { ChoiceOptionDto } from '../protocol/types.ts';
import { getTradePayload } from './choiceModel.ts';

export type TradeCategory =
  | 'commodity_swap'
  | 'goods_exchange'
  | 'promissory'
  | 'mutual_support'
  | 'other';

export interface DecodedTradeOffer {
  id: string;
  category: TradeCategory;
  label: string;
  net?: number;
  their_net?: number;
  details: {
    giveCommodities?: number;
    receiveCommodities?: number;
    giveTradeGoods?: number;
    receiveTradeGoods?: number;
    promissoryNote?: string;
    actionCard?: string;
    secretObjective?: string;
    fragment?: string;
    price?: number;
  };
}

export function decodeTradeOption(opt: ChoiceOptionDto): DecodedTradeOffer {
  const id = opt.id;
  const payload = getTradePayload(opt);
  const net = payload.net;
  const their_net = payload.their_net;

  if (id === 'ss') {
    return {
      id,
      category: 'mutual_support',
      label: opt.label || 'Swap Support for the Throne',
      net,
      their_net,
      details: {},
    };
  }

  if (id.startsWith('cc')) {
    const amount = parseInt(id.slice(2), 10) || 1;
    return {
      id,
      category: 'commodity_swap',
      label: opt.label || `Swap ${amount} commodities each`,
      net,
      their_net,
      details: {
        giveCommodities: amount,
        receiveCommodities: amount,
      },
    };
  }

  if (id.startsWith('ct')) {
    const parts = id.slice(2).split(':');
    const give = parseInt(parts[0], 10) || 0;
    const want = parseInt(parts[1], 10) || 0;
    return {
      id,
      category: 'goods_exchange',
      label: opt.label || `Give ${give} commodities for ${want} trade goods`,
      net,
      their_net,
      details: {
        giveCommodities: give,
        receiveTradeGoods: want,
      },
    };
  }

  if (id.startsWith('tc')) {
    const parts = id.slice(2).split(':');
    const give = parseInt(parts[0], 10) || 0;
    const want = parseInt(parts[1], 10) || 0;
    return {
      id,
      category: 'goods_exchange',
      label: opt.label || `Give ${give} trade goods for ${want} commodities`,
      net,
      their_net,
      details: {
        giveTradeGoods: give,
        receiveCommodities: want,
      },
    };
  }

  if (id.startsWith('c') && id.includes(':')) {
    const parts = id.slice(1).split(':');
    const give = parseInt(parts[0], 10) || 0;
    return {
      id,
      category: 'commodity_swap',
      label: opt.label || `Gift ${give} commodities`,
      net,
      their_net,
      details: {
        giveCommodities: give,
      },
    };
  }

  if (id.startsWith('pn')) {
    const rest = id.slice(2);
    const lastColon = rest.lastIndexOf(':');
    const note = lastColon !== -1 ? rest.slice(0, lastColon) : rest;
    const price = lastColon !== -1 ? parseInt(rest.slice(lastColon + 1), 10) || 0 : 0;
    return {
      id,
      category: 'promissory',
      label: opt.label || `Promissory Note: ${note} for ${price} TG`,
      net,
      their_net,
      details: {
        promissoryNote: note,
        price,
      },
    };
  }

  if (id.startsWith('ac')) {
    const parts = id.slice(2).split(':');
    const card = parts[0];
    const price = parseInt(parts[1], 10) || 0;
    return {
      id,
      category: 'other',
      label: opt.label || `Action Card: ${card} for ${price} TG`,
      net,
      their_net,
      details: {
        actionCard: card,
        price,
      },
    };
  }

  if (id.startsWith('so')) {
    const parts = id.slice(2).split(':');
    const secret = parts[0];
    const price = parseInt(parts[1], 10) || 0;
    return {
      id,
      category: 'other',
      label: opt.label || `Secret Objective: ${secret} for ${price} TG`,
      net,
      their_net,
      details: {
        secretObjective: secret,
        price,
      },
    };
  }

  if (id.startsWith('fr')) {
    const parts = id.slice(2).split(':');
    const trait = parts[0];
    const price = parseInt(parts[1], 10) || 0;
    return {
      id,
      category: 'other',
      label: opt.label || `Fragment (${trait}) for ${price} TG`,
      net,
      their_net,
      details: {
        fragment: trait,
        price,
      },
    };
  }

  if (id.includes(':')) {
    const parts = id.split(':');
    const give = parseInt(parts[0], 10) || 0;
    const want = parseInt(parts[1], 10) || 0;
    return {
      id,
      category: 'goods_exchange',
      label: opt.label || `Give ${give} trade goods for ${want} trade goods`,
      net,
      their_net,
      details: {
        giveTradeGoods: give,
        receiveTradeGoods: want,
      },
    };
  }

  return {
    id,
    category: 'other',
    label: opt.label,
    net,
    their_net,
    details: {},
  };
}
