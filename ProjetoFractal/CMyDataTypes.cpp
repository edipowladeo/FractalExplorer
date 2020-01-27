#include "Ccoordenadas.h"


bool operator ==(const CcoordenadasPlano& C1, const CcoordenadasPlano& C2)
{
    return(C1.x == C2.x && C1.y == C2.y && C1.delta == C2.delta);
}